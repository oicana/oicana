//! Shared core logic for Oicana FFI integrations.
//!
//! Each language integration (node, python, php, java, csharp, browser-wasm) is
//! a thin shim around the functions in this crate. The shim handles marshaling
//! between language-native types and the neutral types exposed here, then calls
//! into the corresponding function in this crate.

use std::collections::HashMap;
use std::io::Cursor;
use std::str::FromStr;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};

use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use typst::foundations::Bytes;
use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};
use typst_layout::PagedDocument;
use uuid::Uuid;

pub use oicana_export::pages::PageRange;
use oicana_export::pdf::export_pdf;
use oicana_export::png::{export_png, PngExportError};
use oicana_export::svg::{export_svg, SvgExportError};
use oicana_export::PdfStandard;
use oicana_files::packed::PackedTemplate;
pub use oicana_files::packed::{NegativeZipLimit, ZipLimits};
use oicana_files::TemplateFiles;
use oicana_input::input::blob::{Blob, BlobInput};
use oicana_input::input::json::JsonInput;
use oicana_input::{CompilationConfig, TemplateInputs};
use oicana_world::diagnostics::PlainDiagnostics;
use oicana_world::fonts::FontSource;
use oicana_world::manifest::OicanaWorldFiles;
use oicana_world::world::OicanaWorld;

/// Diagnostic-output coloring (re-exported from `oicana_world`).
pub use oicana_world::diagnostics::DiagnosticColor;

/// The template manifest
pub mod manifest;

/// Compilation mode passed in from the calling language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompilationMode {
    /// Required inputs that are not provided cause compilation to fail.
    Production,
    /// Required inputs that are not provided fall back to development or default values.
    Development,
}

impl From<CompilationMode> for CompilationConfig {
    fn from(value: CompilationMode) -> Self {
        match value {
            CompilationMode::Production => CompilationConfig::production(),
            CompilationMode::Development => CompilationConfig::development(),
        }
    }
}

/// A blob input value together with its JSON-encoded metadata.
///
/// Integrations should construct one of these per blob input, where `meta`
/// is a JSON string (`"{}"` if no metadata is provided).
pub struct BlobWithMetadata {
    /// Raw bytes of the blob.
    pub bytes: Vec<u8>,
    /// JSON-encoded metadata associated with the blob.
    pub meta: String,
}

/// Export format and parameters for a single document export.
#[derive(Debug, Deserialize)]
#[serde(tag = "format")]
pub enum ExportFormat {
    /// PNG export with configurable resolution.
    #[serde(alias = "png")]
    Png {
        /// Pixels per point used for rendering.
        #[serde(rename = "pixelsPerPt")]
        pixels_per_pt: f32,
    },
    /// PDF export. The PDF standards are taken from the template manifest.
    #[serde(alias = "pdf")]
    Pdf,
    /// SVG export.
    #[serde(alias = "svg")]
    Svg,
}

/// All errors that core functions can return.
///
/// Integrations should map this to their language-native error type via
/// `.to_string()` (or via individual variant matching when finer control is desired).
#[derive(Debug, Error)]
pub enum FfiError {
    /// The requested template ID is not present in the world cache.
    #[error("Template '{0}' is not registered")]
    TemplateNotRegistered(String),

    /// The requested document ID is not present in the document cache.
    #[error("Document '{0}' not found")]
    DocumentNotFound(String),

    /// The document ID does not match the expected `<uuid>:<template_id>` format.
    #[error("Invalid document ID format: {0}")]
    InvalidDocumentId(String),

    /// The packed template bytes could not be read.
    #[error("Failed to read template: {0}")]
    PackedTemplate(String),

    /// The template manifest could not be parsed.
    #[error("Manifest error: {0}")]
    Manifest(String),

    /// Creating the Typst world for the template failed.
    #[error("World creation error: {0}")]
    WorldCreation(String),

    /// Updating inputs on an existing world failed (typically schema validation).
    #[error("Input validation failed: {0}")]
    InputValidation(String),

    /// Template compilation failed.
    #[error("Compilation failed: {0}")]
    Compilation(String),

    /// Encoding the compiled document to the requested format failed.
    #[error("Failed to encode {format}: {error}")]
    Export {
        /// Human-readable name of the target format ("PDF", "PNG", "SVG").
        format: &'static str,
        /// Underlying encoder error.
        error: String,
    },

    /// Loading the source for a file path failed.
    #[error("Failed to load source '{path}': {error}")]
    SourceLoad {
        /// Requested path inside the template.
        path: String,
        /// Underlying file-system error.
        error: String,
    },

    /// Loading the bytes for a file path failed.
    #[error("Failed to load file '{path}': {error}")]
    FileLoad {
        /// Requested path inside the template.
        path: String,
        /// Underlying file-system error.
        error: String,
    },

    /// Serializing the template's manifest to JSON failed.
    #[error("Failed to serialize the manifest: {0}")]
    ManifestSerialization(String),

    /// Serializing the document's page sizes to JSON failed.
    #[error("Failed to serialize page sizes: {0}")]
    PageSizesSerialization(String),

    /// The blob metadata for a key was not valid JSON or did not match the metadata schema.
    #[error("Failed to parse blob metadata for '{key}': {error}")]
    BlobMetadata {
        /// Blob input key whose metadata failed to parse.
        key: String,
        /// Underlying serde error.
        error: String,
    },

    /// The `export_format` JSON could not be parsed into an [`ExportFormat`].
    #[error("Failed to parse export format: {0}")]
    ExportFormatParse(String),

    /// The `page_range` JSON could not be parsed into a [`PageRange`].
    #[error("Failed to parse page range: {0}")]
    PageRangeParse(String),
}

impl From<PngExportError> for FfiError {
    fn from(error: PngExportError) -> Self {
        FfiError::Export {
            format: "PNG",
            error: format!("{error:?}"),
        }
    }
}

impl From<SvgExportError> for FfiError {
    fn from(error: SvgExportError) -> Self {
        FfiError::Export {
            format: "SVG",
            error: format!("{error:?}"),
        }
    }
}

/// A compiled document together with the PDF export settings captured from its
/// template's manifest at compile time.
struct CachedDocument {
    document: PagedDocument,
    pdf_standards: Vec<PdfStandard>,
    pdf_tagged: bool,
}

/// A registered world behind its own per-template lock.
type SharedWorld = Arc<RwLock<OicanaWorld<PackedTemplate>>>;

// `WORLD_CACHE` and `DOCUMENT_CACHE` shard guards are held only long enough
// to clone the `Arc` out. Blocking lock order for everything else:
// world `RwLock` -> `WARNINGS_CACHE`, at most one guard per map at a time.
// Violations can deadlock integrations that call in from multiple threads.
static WORLD_CACHE: Lazy<DashMap<String, SharedWorld>> = Lazy::new(DashMap::new);
static DOCUMENT_CACHE: Lazy<DashMap<String, Arc<CachedDocument>>> = Lazy::new(DashMap::new);
static WARNINGS_CACHE: Lazy<DashMap<String, String>> = Lazy::new(DashMap::new);

fn get_world(template_id: &str) -> Result<SharedWorld, FfiError> {
    WORLD_CACHE
        .get(template_id)
        .map(|entry| Arc::clone(entry.value()))
        .ok_or_else(|| FfiError::TemplateNotRegistered(template_id.to_owned()))
}

fn read_world(
    world: &RwLock<OicanaWorld<PackedTemplate>>,
) -> RwLockReadGuard<'_, OicanaWorld<PackedTemplate>> {
    world.read().unwrap_or_else(PoisonError::into_inner)
}

fn write_world(
    world: &RwLock<OicanaWorld<PackedTemplate>>,
) -> RwLockWriteGuard<'_, OicanaWorld<PackedTemplate>> {
    world.write().unwrap_or_else(PoisonError::into_inner)
}

fn try_read_world(
    world: &RwLock<OicanaWorld<PackedTemplate>>,
) -> Option<RwLockReadGuard<'_, OicanaWorld<PackedTemplate>>> {
    match world.try_read() {
        Ok(guard) => Some(guard),
        Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
        Err(TryLockError::WouldBlock) => None,
    }
}

/// Cache age threshold for automatic eviction. `usize::MAX` means disabled.
/// Default keeps entries used within the last 10 evictions.
static CACHE_EVICTION_AGE: AtomicUsize = AtomicUsize::new(10);

/// Global diagnostic-color setting applied to every world created via
/// [`register_template`]. Defaults to `None`.
/// Encoded as 0 = `None`, 1 = `Ansi`.
static DIAGNOSTIC_COLOR: AtomicU8 = AtomicU8::new(0);

fn current_diagnostic_color() -> DiagnosticColor {
    match DIAGNOSTIC_COLOR.load(Ordering::Relaxed) {
        1 => DiagnosticColor::Ansi,
        _ => DiagnosticColor::None,
    }
}

/// Configure the diagnostic-output coloring used for compilation diagnostics.
///
/// The setting is applied immediately to every cached world and used for
/// every world created after this call.
pub fn configure_diagnostic_color(color: DiagnosticColor) {
    DIAGNOSTIC_COLOR.store(
        match color {
            DiagnosticColor::None => 0,
            DiagnosticColor::Ansi => 1,
        },
        Ordering::Relaxed,
    );
    // Collect the `Arc`s first so no shard guard is held while waiting on a
    // world that is mid-compilation.
    let worlds: Vec<SharedWorld> = WORLD_CACHE
        .iter()
        .map(|entry| Arc::clone(entry.value()))
        .collect();
    for world in worlds {
        write_world(&world).color = current_diagnostic_color();
    }
}

/// Fonts registered by the host, shared by every world built after they were
/// added.
///
/// Registering fonts is a process-wide setting. Worlds snapshot the list when they are created, so
/// fonts have to be registered before the templates that need them.
static HOST_FONTS: Lazy<RwLock<Vec<FontSource>>> = Lazy::new(|| RwLock::new(Vec::new()));

fn host_fonts() -> Vec<FontSource> {
    HOST_FONTS
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

fn add_host_fonts(sources: Vec<FontSource>) -> usize {
    let faces = sources
        .iter()
        .map(|source| source.families().len())
        .sum::<usize>();
    let mut fonts = HOST_FONTS.write().unwrap_or_else(PoisonError::into_inner);
    fonts.extend(sources);
    if !WORLD_CACHE.is_empty() {
        log::warn!(
            "Fonts were registered while {} template(s) are already registered; \
             those templates keep the fonts they were created with. \
             Register fonts before registering templates.",
            WORLD_CACHE.len()
        );
    }
    faces
}

/// A font face made available to templates by the host.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RegisteredFont {
    /// The family name.
    pub family: String,
    /// The file the face was read from, if it was registered by path.
    pub path: Option<String>,
}

/// Register fonts from their raw file content.
///
/// Register each font only once: nothing deduplicates.
///
/// Returns the number of font faces that were added.
pub fn register_fonts(fonts: Vec<Vec<u8>>) -> usize {
    let sources = fonts
        .into_iter()
        .filter_map(|data| FontSource::from_bytes(Bytes::new(data)))
        .collect();
    add_host_fonts(sources)
}

/// Register a single font from its raw file content.
///
/// Returns the number of font faces that were added.
pub fn register_font(data: Vec<u8>) -> usize {
    register_fonts(vec![data])
}

/// Register fonts from paths on disk.
///
/// Each path is either a font file or a directory, which adds every `.ttf`, `.ttc`,
/// `.otf`, and `.otc` file in its tree.
///
/// The files are read immediately to collect their face metadata and then dropped; each
/// is read again, and kept, the first time a document needs one of its glyphs.
///
/// Returns the number of font faces that were added.
#[cfg(not(target_arch = "wasm32"))]
pub fn register_font_paths(paths: Vec<std::path::PathBuf>) -> usize {
    let sources = paths
        .into_iter()
        .flat_map(oicana_world::fonts::font_files_at)
        .filter_map(FontSource::from_path)
        .collect();
    add_host_fonts(sources)
}

/// All font faces currently registered by the host.
pub fn registered_fonts() -> Vec<RegisteredFont> {
    host_fonts()
        .iter()
        .flat_map(|source| {
            let path = source
                .path()
                .map(|path| path.to_string_lossy().into_owned());
            source
                .families()
                .into_iter()
                .map(move |family| RegisteredFont {
                    family,
                    path: path.clone(),
                })
        })
        .collect()
}

/// Drop all fonts registered by the host.
///
/// Templates that are already registered keep the fonts they were created with.
pub fn clear_fonts() {
    HOST_FONTS
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
}

/// Parse an `export_format` JSON string into an [`ExportFormat`].
///
/// Integrations that receive the export format as a JSON string (csharp, node,
/// php, python, java) can use this; integrations that deserialize directly
/// from a language-native value (browser-wasm via serde-wasm-bindgen) can
/// build [`ExportFormat`] themselves and skip this.
pub fn parse_export_format(json: &str) -> Result<ExportFormat, FfiError> {
    serde_json::from_str(json).map_err(|error| FfiError::ExportFormatParse(error.to_string()))
}

/// Parse an optional [`PageRange`] from its JSON object representation.
///
/// `None` means "no range" (the whole document). Otherwise the JSON is a
/// `{ "start"?: number, "end"?: number }` object with 0-based, inclusive
/// bounds.
pub fn parse_page_range(json: Option<&str>) -> Result<Option<PageRange>, FfiError> {
    let Some(json) = json else {
        return Ok(None);
    };
    serde_json::from_str(json)
        .map(Some)
        .map_err(|error| FfiError::PageRangeParse(error.to_string()))
}

/// Register a template under the given identifier.
///
/// Reads `files` as a [`PackedTemplate`], builds a Typst world, compiles
/// once with the given inputs as a warm-up, and stores both the world and
/// the resulting document. Returns the document ID for the warm-up result.
///
/// Subsequent calls with the same `template_id` can use
/// [`compile_template`] without re-reading the files.
pub fn register_template(
    template_id: &str,
    files: &[u8],
    json_inputs: HashMap<String, String>,
    blob_inputs: HashMap<String, BlobWithMetadata>,
    mode: CompilationMode,
    limits: Option<ZipLimits>,
) -> Result<String, FfiError> {
    let packed = PackedTemplate::new_with_limits(Cursor::new(files), limits.unwrap_or_default())
        .map_err(|error| FfiError::PackedTemplate(error.to_string()))?;
    let manifest = packed
        .manifest()
        .map_err(|error| FfiError::Manifest(error.to_string()))?;

    let mut inputs = prepare_inputs(json_inputs, blob_inputs)?;
    inputs.with_config(mode.into());

    let mut world = OicanaWorld::new_with_fonts(packed, inputs, manifest, &host_fonts())
        .map_err(|error| FfiError::WorldCreation(error.to_string()))?;
    world.color = current_diagnostic_color();

    let document = world
        .compile()
        .map_err(|error| FfiError::Compilation(error.to_string()))?;

    let pdf_standards = world.manifest().pdf_standards().to_vec();
    let pdf_tagged = world.manifest().pdf_tagged();
    let result_id = new_document_id(template_id);
    WORLD_CACHE.insert(template_id.to_owned(), Arc::new(RwLock::new(world)));
    store_warnings(&result_id, document.warnings);
    DOCUMENT_CACHE.insert(
        result_id.clone(),
        Arc::new(CachedDocument {
            document: document.document,
            pdf_standards,
            pdf_tagged,
        }),
    );

    auto_evict();

    Ok(result_id)
}

/// Compile the previously-registered template with the given inputs.
///
/// Requires a prior successful call to [`register_template`] with the
/// same `template_id`. Returns the document ID of the new compilation.
pub fn compile_template(
    template_id: &str,
    json_inputs: HashMap<String, String>,
    blob_inputs: HashMap<String, BlobWithMetadata>,
    mode: CompilationMode,
) -> Result<String, FfiError> {
    let shared = get_world(template_id)?;

    let mut inputs = prepare_inputs(json_inputs, blob_inputs)?;
    inputs.with_config(mode.into());

    let mut world = write_world(&shared);
    world
        .update_inputs(inputs)
        .map_err(|error| FfiError::InputValidation(error.to_string()))?;

    let document = world
        .compile()
        .map_err(|error| FfiError::Compilation(error.to_string()))?;

    let pdf_standards = world.manifest().pdf_standards().to_vec();
    let pdf_tagged = world.manifest().pdf_tagged();
    // We can free the lock on the world early.
    drop(world);

    let result_id = new_document_id(template_id);
    store_warnings(&result_id, document.warnings);
    DOCUMENT_CACHE.insert(
        result_id.clone(),
        Arc::new(CachedDocument {
            document: document.document,
            pdf_standards,
            pdf_tagged,
        }),
    );

    auto_evict();

    Ok(result_id)
}

/// Export result bytes with their compilation warnings.
#[derive(Debug)]
pub struct ExportOnceResult {
    /// The exported document.
    pub bytes: Vec<u8>,
    /// Compilation warnings, if any.
    pub warnings: Option<String>,
}

/// Compile a template once and immediately export it, without caching the world.
///
/// Useful for one-off exports where the template will not be reused.
/// For repeated compilations of the same template, use [`register_template`]
/// + [`compile_template`] + [`export_document`] instead.
pub fn export_once(
    files: &[u8],
    json_inputs: HashMap<String, String>,
    blob_inputs: HashMap<String, BlobWithMetadata>,
    mode: CompilationMode,
    format: ExportFormat,
    pages: Option<PageRange>,
    limits: Option<ZipLimits>,
) -> Result<ExportOnceResult, FfiError> {
    let packed = PackedTemplate::new_with_limits(Cursor::new(files), limits.unwrap_or_default())
        .map_err(|error| FfiError::PackedTemplate(error.to_string()))?;
    let manifest = packed
        .manifest()
        .map_err(|error| FfiError::Manifest(error.to_string()))?;

    let mut inputs = prepare_inputs(json_inputs, blob_inputs)?;
    inputs.with_config(mode.into());

    let mut world = OicanaWorld::new_with_fonts(packed, inputs, manifest, &host_fonts())
        .map_err(|error| FfiError::WorldCreation(error.to_string()))?;
    world.color = current_diagnostic_color();

    let document = world
        .compile()
        .map_err(|error| FfiError::Compilation(error.to_string()))?;

    let warnings = document.warnings;
    let document = &document.document;
    let bytes = match format {
        ExportFormat::Png { pixels_per_pt } => {
            export_png(document, pixels_per_pt, pages.as_ref()).map_err(FfiError::from)
        }
        ExportFormat::Pdf => export_pdf(
            document,
            &world,
            world.manifest().pdf_standards(),
            world.manifest().pdf_tagged(),
            pages.as_ref(),
        )
        .map_err(pdf_export_error),
        ExportFormat::Svg => export_svg(document, pages.as_ref()).map_err(FfiError::from),
    };

    auto_evict();

    Ok(ExportOnceResult {
        bytes: bytes?,
        warnings,
    })
}

/// Export a previously-compiled document.
pub fn export_document(
    document_id: &str,
    format: ExportFormat,
    pages: Option<PageRange>,
) -> Result<Vec<u8>, FfiError> {
    let shared = if matches!(format, ExportFormat::Pdf) {
        let template_id = template_id_from_document_id(document_id)?;
        WORLD_CACHE
            .get(template_id)
            .map(|entry| Arc::clone(entry.value()))
    } else {
        None
    };
    let world = shared.as_ref().and_then(|world| try_read_world(world));

    let Some(cached) = DOCUMENT_CACHE
        .get(document_id)
        .map(|entry| Arc::clone(entry.value()))
    else {
        return Err(FfiError::DocumentNotFound(document_id.to_owned()));
    };

    Ok(match format {
        ExportFormat::Png { pixels_per_pt } => {
            export_png(&cached.document, pixels_per_pt, pages.as_ref())?
        }
        ExportFormat::Pdf => {
            // Fall back to plain (span-less) diagnostics when the world is not available.
            match world.as_deref() {
                Some(world) => export_pdf(
                    &cached.document,
                    world,
                    &cached.pdf_standards,
                    cached.pdf_tagged,
                    pages.as_ref(),
                ),
                None => export_pdf(
                    &cached.document,
                    &PlainDiagnostics,
                    &cached.pdf_standards,
                    cached.pdf_tagged,
                    pages.as_ref(),
                ),
            }
            .map_err(pdf_export_error)?
        }
        ExportFormat::Svg => export_svg(&cached.document, pages.as_ref())?,
    })
}

/// Size of a single document page, in typographic points (pt).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PageSize {
    /// Page width in points.
    pub width: f64,
    /// Page height in points.
    pub height: f64,
}

/// Return the sizes (in points) of every page of a previously-compiled document,
/// serialized as a JSON array of `{ "width": f64, "height": f64 }`.
pub fn document_pages(document_id: &str) -> Result<String, FfiError> {
    let Some(cached) = DOCUMENT_CACHE
        .get(document_id)
        .map(|entry| Arc::clone(entry.value()))
    else {
        return Err(FfiError::DocumentNotFound(document_id.to_owned()));
    };

    let pages: Vec<PageSize> = cached
        .document
        .pages()
        .iter()
        .map(|page| {
            let size = page.frame.size();
            PageSize {
                width: size.x.to_pt(),
                height: size.y.to_pt(),
            }
        })
        .collect();

    serde_json::to_string(&pages)
        .map_err(|error| FfiError::PageSizesSerialization(error.to_string()))
}

/// Return the template's manifest serialized as a JSON string.
pub fn manifest(template_id: &str) -> Result<String, FfiError> {
    let shared = get_world(template_id)?;
    let world = read_world(&shared);
    let manifest = manifest::Manifest::from(world.manifest());
    serde_json::to_string(&manifest)
        .map_err(|error| FfiError::ManifestSerialization(error.to_string()))
}

/// Return the source text of a file inside the template.
pub fn get_source(template_id: &str, path: &str) -> Result<String, FfiError> {
    let shared = get_world(template_id)?;
    let world = read_world(&shared);
    let vpath = VirtualPath::new(path).map_err(|error| FfiError::SourceLoad {
        path: path.to_owned(),
        error: error.to_string(),
    })?;
    world
        .files
        .source(FileId::new(RootedPath::new(VirtualRoot::Project, vpath)))
        .map(|source| source.text().to_string())
        .map_err(|error| FfiError::SourceLoad {
            path: path.to_owned(),
            error: error.to_string(),
        })
}

/// Return the raw bytes of a file inside the template.
pub fn get_file(template_id: &str, path: &str) -> Result<Vec<u8>, FfiError> {
    let shared = get_world(template_id)?;
    let world = read_world(&shared);
    let vpath = VirtualPath::new(path).map_err(|error| FfiError::FileLoad {
        path: path.to_owned(),
        error: error.to_string(),
    })?;
    world
        .files
        .file(FileId::new(RootedPath::new(VirtualRoot::Project, vpath)))
        .map(|bytes| bytes.to_vec())
        .map_err(|error| FfiError::FileLoad {
            path: path.to_owned(),
            error: error.to_string(),
        })
}

/// Enable or disable JSON schema validation of inputs for a registered template.
///
/// When enabled (the default), JSON inputs are validated against their schemas
/// before compilation.
pub fn set_validate_inputs(template_id: &str, validate: bool) -> Result<(), FfiError> {
    let shared = get_world(template_id)?;
    write_world(&shared).validate_inputs = validate;
    Ok(())
}

/// Drop the cached document (and any warnings stored alongside it), freeing
/// its memory.
pub fn remove_document(document_id: &str) {
    DOCUMENT_CACHE.remove(document_id);
    WARNINGS_CACHE.remove(document_id);
}

/// Return any compilation warnings produced for the given document.
///
/// Warnings are stored when [`register_template`] or [`compile_template`]
/// successfully produce a document. They are cleared together with the
/// document by [`remove_document`].
pub fn get_warnings(document_id: &str) -> Option<String> {
    WARNINGS_CACHE.get(document_id).map(|entry| entry.clone())
}

fn store_warnings(document_id: &str, warnings: Option<String>) {
    if let Some(warnings) = warnings {
        WARNINGS_CACHE.insert(document_id.to_owned(), warnings);
    }
}

/// Drop the cached world for a template. Subsequent compilations require
/// a fresh [`register_template`] call.
pub fn remove_world(template_id: &str) {
    WORLD_CACHE.remove(template_id);
}

/// Configure automatic comemo cache eviction after each compilation.
///
/// `max_age` semantics:
///   - `None` – disable automatic eviction (the cache is never cleared).
///   - `Some(0)` – clear every cache entry on each eviction.
///   - `Some(1)` – keep only entries used since the last eviction.
///   - `Some(n)` – keep entries used within the last `n` evictions.
///
/// The default is `Some(10)`.
pub fn configure_automatic_cache_eviction(max_age: Option<usize>) {
    CACHE_EVICTION_AGE.store(max_age.unwrap_or(usize::MAX), Ordering::Relaxed);
}

/// Manually evict the comemo cache with the given age threshold, regardless
/// of the automatic eviction setting.
pub fn evict_cache(max_age: usize) {
    oicana_world::evict_cache(max_age);
}

/// Extract a human-readable message from a panic payload as returned by
/// [`std::panic::catch_unwind`].
///
/// Integrations must not let panics unwind across their language boundary.
/// Catch them at every entry point and use this
/// to turn the payload into a message for the language-native error channel.
pub fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(message) = payload.downcast_ref::<&str>() {
        message
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message
    } else {
        "unknown cause"
    }
}

/// Run `body`, discarding any panic. For FFI entry points that have no
/// error channel to report through.
pub fn swallow_panic(body: impl FnOnce()) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
}

fn auto_evict() {
    let cache_age = CACHE_EVICTION_AGE.load(Ordering::Relaxed);
    if cache_age != usize::MAX {
        oicana_world::evict_cache(cache_age);
    }
}

fn new_document_id(template_id: &str) -> String {
    format!("{}:{}", Uuid::new_v4(), template_id)
}

/// UUID v4 string representations are 36 chars, so a document ID is at least
/// 38 chars (`<uuid>:<at-least-one-char>`).
const DOCUMENT_ID_UUID_LEN: usize = 36;

fn template_id_from_document_id(document_id: &str) -> Result<&str, FfiError> {
    let bytes = document_id.as_bytes();
    if bytes.len() <= DOCUMENT_ID_UUID_LEN + 1 || bytes[DOCUMENT_ID_UUID_LEN] != b':' {
        return Err(FfiError::InvalidDocumentId(document_id.to_owned()));
    }
    Ok(&document_id[DOCUMENT_ID_UUID_LEN + 1..])
}

fn prepare_inputs(
    json_inputs: HashMap<String, String>,
    blob_inputs: HashMap<String, BlobWithMetadata>,
) -> Result<TemplateInputs, FfiError> {
    let mut inputs = TemplateInputs::new();

    for (key, value) in json_inputs {
        inputs.with_input(JsonInput::new(key, value));
    }

    for (key, value) in blob_inputs {
        let mut blob = Blob::from(Bytes::new(value.bytes));
        let parsed =
            serde_json::Value::from_str(&value.meta).map_err(|error| FfiError::BlobMetadata {
                key: key.clone(),
                error: error.to_string(),
            })?;
        blob.metadata =
            Deserialize::deserialize(parsed).map_err(|error| FfiError::BlobMetadata {
                key: key.clone(),
                error: error.to_string(),
            })?;
        inputs.with_input(BlobInput::new(key, blob));
    }

    Ok(inputs)
}

/// Map the bare `String` error from [`export_pdf`] to an [`FfiError`].
fn pdf_export_error(error: String) -> FfiError {
    FfiError::Export {
        format: "PDF",
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_fonts_can_be_registered_listed_and_cleared() {
        let font = typst_assets::fonts().next().expect("typst ships fonts");

        assert!(registered_fonts().is_empty());

        let faces = register_fonts(vec![font.to_vec()]);
        assert!(faces > 0);
        let registered = registered_fonts();
        assert_eq!(registered.len(), faces);
        assert!(registered.iter().all(|font| font.path.is_none()));
        assert!(registered.iter().all(|font| !font.family.is_empty()));

        assert_eq!(register_fonts(vec![b"not a font".to_vec()]), 0);
        assert_eq!(registered_fonts().len(), faces);

        clear_fonts();
        assert!(registered_fonts().is_empty());

        // A path is either a font file or a directory holding font files.
        let dir = std::env::temp_dir().join("oicana_ffi_font_dir_test");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("regular.ttf"), font).unwrap();
        std::fs::write(dir.join("nested/nested.ttf"), font).unwrap();
        std::fs::write(dir.join("readme.txt"), b"not a font").unwrap();

        assert_eq!(register_font_paths(vec![dir.join("regular.ttf")]), faces);
        clear_fonts();

        assert_eq!(register_font_paths(vec![dir.clone()]), faces * 2);
        assert!(registered_fonts().iter().all(|font| font.path.is_some()));
        clear_fonts();

        assert_eq!(register_font_paths(vec![dir.join("missing")]), 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parses_export_format_pdf() {
        let format = parse_export_format(r#"{"format":"pdf"}"#).unwrap();
        assert!(matches!(format, ExportFormat::Pdf));
    }

    #[test]
    fn parses_export_format_png_with_resolution() {
        let format = parse_export_format(r#"{"format":"png","pixelsPerPt":2.0}"#).unwrap();
        match format {
            ExportFormat::Png { pixels_per_pt } => assert_eq!(pixels_per_pt, 2.0),
            other => panic!("expected PNG, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_export_format() {
        let err = parse_export_format(r#"{"format":"docx"}"#).unwrap_err();
        assert!(matches!(err, FfiError::ExportFormatParse(_)));
    }

    #[test]
    fn parses_absent_page_range_as_none() {
        assert_eq!(parse_page_range(None).unwrap(), None);
    }

    #[test]
    fn rejects_an_empty_page_range() {
        let err = parse_page_range(Some("")).unwrap_err();
        assert!(matches!(err, FfiError::PageRangeParse(_)));
    }

    #[test]
    fn parses_page_range_bounds() {
        assert_eq!(
            parse_page_range(Some(r#"{"start":2,"end":3}"#)).unwrap(),
            Some(PageRange {
                start: Some(2),
                end: Some(3),
            })
        );
        assert_eq!(
            parse_page_range(Some(r#"{"start":2}"#)).unwrap(),
            Some(PageRange::from(2))
        );
        assert_eq!(
            parse_page_range(Some(r#"{"end":4}"#)).unwrap(),
            Some(PageRange::to(4))
        );
        assert_eq!(
            parse_page_range(Some("{}")).unwrap(),
            Some(PageRange::default())
        );
    }

    #[test]
    fn rejects_invalid_page_range() {
        let err = parse_page_range(Some(r#"{"start":"two"}"#)).unwrap_err();
        assert!(matches!(err, FfiError::PageRangeParse(_)));
    }

    #[test]
    fn template_id_round_trips_through_document_id() {
        let doc_id = new_document_id("some-template");
        assert_eq!(
            template_id_from_document_id(&doc_id).unwrap(),
            "some-template"
        );
    }

    #[test]
    fn rejects_document_id_without_separator() {
        let err = template_id_from_document_id("not-a-document-id").unwrap_err();
        assert!(matches!(err, FfiError::InvalidDocumentId(_)));
    }

    #[test]
    fn rejects_document_id_without_template_id_suffix() {
        let only_uuid = format!("{}", Uuid::new_v4());
        let err = template_id_from_document_id(&only_uuid).unwrap_err();
        assert!(matches!(err, FfiError::InvalidDocumentId(_)));

        let trailing_colon = format!("{}:", Uuid::new_v4());
        let err = template_id_from_document_id(&trailing_colon).unwrap_err();
        assert!(matches!(err, FfiError::InvalidDocumentId(_)));
    }

    #[test]
    fn manifest_is_serialized_in_the_documented_shape() {
        let files = std::fs::read("../../assets/templates/invoice-0.1.0.zip")
            .expect("read test template fixture");
        let template_id = format!("manifest-shape-{}", Uuid::new_v4());
        let document_id = register_template(
            &template_id,
            &files,
            HashMap::new(),
            HashMap::new(),
            CompilationMode::Development,
            None,
        )
        .expect("register template");
        remove_document(&document_id);

        let serialized = manifest(&template_id).expect("serialize manifest");
        remove_world(&template_id);

        let parsed: serde_json::Value =
            serde_json::from_str(&serialized).expect("manifest is valid JSON");

        // Every integration declares this shape in its own language, so a
        // change here has to be mirrored in all of them.
        assert_eq!(
            parsed,
            serde_json::json!({
                "package": {
                    "name": "invoice",
                    "version": "0.1.0",
                    "entrypoint": "main.typ",
                    "authors": [],
                    "license": null,
                    "description": "Invoice template with customizable items and billing details.",
                    "homepage": null,
                    "repository": null
                },
                "oicana": {
                    "manifestVersion": 1,
                    "inputs": [
                        {
                            "type": "json",
                            "key": "invoice",
                            "required": true,
                            "default": "invoice.json",
                            "development": null,
                            "schema": "invoice.schema.json",
                            "validate": true
                        },
                        {
                            "type": "blob",
                            "key": "banner",
                            "required": true,
                            "default": {
                                "file": "oicana.png",
                                "meta": { "image_format": "png" }
                            },
                            "development": null
                        }
                    ],
                    "validateJsonInputsByDefault": true,
                    "export": {
                        "pdf": { "standards": ["a-3b"], "tagged": true }
                    },
                    "fonts": { "require": [] }
                }
            })
        );
    }

    #[test]
    fn registering_a_template_from_a_newer_oicana_fails() {
        let files = std::fs::read("../../assets/templates/future-manifest-0.1.0.zip")
            .expect("read test template fixture");

        let error = register_template(
            &format!("future-manifest-{}", Uuid::new_v4()),
            &files,
            HashMap::new(),
            HashMap::new(),
            CompilationMode::Development,
            None,
        )
        .expect_err("a template declaring a newer manifest version must not register");

        assert!(
            error.to_string().contains("manifest_version 99"),
            "got: {error}"
        );
    }

    #[test]
    fn concurrent_compile_and_export_do_not_deadlock() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Duration;

        let files = std::fs::read("../../assets/templates/table-0.1.0.zip")
            .expect("read test template fixture");
        let template_id = format!("concurrency-test-{}", Uuid::new_v4());

        let exported_doc = register_template(
            &template_id,
            &files,
            HashMap::new(),
            HashMap::new(),
            CompilationMode::Development,
            None,
        )
        .expect("register template");

        let (done_tx, done_rx) = mpsc::channel();

        let compile_done = done_tx.clone();
        let compile_template_id = template_id.clone();
        thread::spawn(move || {
            for _ in 0..150 {
                let doc_id = compile_template(
                    &compile_template_id,
                    HashMap::new(),
                    HashMap::new(),
                    CompilationMode::Development,
                )
                .expect("compile template");
                remove_document(&doc_id);
            }
            compile_done.send(()).unwrap();
        });

        let export_doc_id = exported_doc.clone();
        thread::spawn(move || {
            for _ in 0..150 {
                export_document(&export_doc_id, ExportFormat::Pdf, None).expect("export PDF");
            }
            done_tx.send(()).unwrap();
        });

        for _ in 0..2 {
            done_rx
                .recv_timeout(Duration::from_secs(60))
                .expect("concurrent compile/export deadlocked");
        }

        remove_document(&exported_doc);
        remove_world(&template_id);
    }

    #[test]
    fn compile_of_one_template_does_not_block_others() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Duration;

        let files = std::fs::read("../../assets/templates/table-0.1.0.zip")
            .expect("read test template fixture");
        let template_a = format!("blocking-test-a-{}", Uuid::new_v4());
        let template_b = format!("blocking-test-b-{}", Uuid::new_v4());

        for template_id in [&template_a, &template_b] {
            let doc_id = register_template(
                template_id,
                &files,
                HashMap::new(),
                HashMap::new(),
                CompilationMode::Development,
                None,
            )
            .expect("register template");
            remove_document(&doc_id);
        }

        let shared_a = get_world(&template_a).expect("world for template A");
        // take and hold a lock on template a
        let in_flight_compile_of_a = write_world(&shared_a);

        let (done_tx, done_rx) = mpsc::channel();
        let other_template = template_b.clone();
        thread::spawn(move || {
            let doc_id = compile_template(
                &other_template,
                HashMap::new(),
                HashMap::new(),
                CompilationMode::Development,
            )
            .expect("compile template");
            manifest(&other_template).expect("manifest");
            let pdf = export_document(&doc_id, ExportFormat::Pdf, None).expect("export PDF");
            assert!(!pdf.is_empty());
            remove_document(&doc_id);
            done_tx.send(()).unwrap();
        });

        done_rx
            .recv_timeout(Duration::from_secs(60))
            .expect("operations on template B blocked behind template A's in-flight compile");

        drop(in_flight_compile_of_a);
        remove_world(&template_a);
        remove_world(&template_b);
    }

    #[test]
    fn exports_all_formats_after_world_removed() {
        let files = std::fs::read("../../assets/templates/table-0.1.0.zip")
            .expect("read test template fixture");
        let template_id = format!("decouple-test-{}", Uuid::new_v4());

        let doc_id = register_template(
            &template_id,
            &files,
            HashMap::new(),
            HashMap::new(),
            CompilationMode::Development,
            None,
        )
        .expect("register template");

        remove_world(&template_id);

        let pdf = export_document(&doc_id, ExportFormat::Pdf, None)
            .expect("PDF export after world removal");
        assert_eq!(&pdf[0..4], b"%PDF");

        assert!(export_document(&doc_id, ExportFormat::Png { pixels_per_pt: 1.0 }, None).is_ok());
        assert!(export_document(&doc_id, ExportFormat::Svg, None).is_ok());

        remove_document(&doc_id);
    }

    #[test]
    fn panic_message_extracts_static_str_payload() {
        let payload = std::panic::catch_unwind(|| panic!("static message")).unwrap_err();
        assert_eq!(panic_message(payload.as_ref()), "static message");
    }

    #[test]
    fn panic_message_extracts_formatted_string_payload() {
        let payload =
            std::panic::catch_unwind(|| panic!("something went wrong: {}", 42)).unwrap_err();
        assert_eq!(panic_message(payload.as_ref()), "something went wrong: 42");
    }

    #[test]
    fn panic_message_falls_back_for_non_string_payload() {
        let payload = std::panic::catch_unwind(|| std::panic::panic_any(42)).unwrap_err();
        assert_eq!(panic_message(payload.as_ref()), "unknown cause");
    }

    #[test]
    fn swallow_panic_does_not_unwind() {
        swallow_panic(|| panic!("ignored"));
    }

    fn minimal_template_zip(main_typst: &str) -> Vec<u8> {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        use zip::{CompressionMethod, ZipWriter};

        let manifest = concat!(
            "[package]\n",
            "name = \"export-once-test\"\n",
            "version = \"0.1.0\"\n",
            "entrypoint = \"main.typ\"\n",
            "\n",
            "[tool.oicana]\n",
            "manifest_version = 1\n",
        );
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, content) in [("typst.toml", manifest), ("main.typ", main_typst)] {
            writer.start_file(name, options).unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn export_once_surfaces_warnings() {
        let files = minimal_template_zip("#set text(font: \"NonexistentFontFfiCore\")\nContent");

        let result = export_once(
            &files,
            HashMap::new(),
            HashMap::new(),
            CompilationMode::Development,
            ExportFormat::Svg,
            None,
            None,
        )
        .expect("export once");

        assert!(result.bytes.starts_with(b"<svg"));
        let warnings = result.warnings.expect("compile warnings");
        assert!(warnings.contains("NonexistentFontFfiCore"));
    }

    #[test]
    fn export_once_without_warnings_returns_none() {
        let files = minimal_template_zip("Content");

        let result = export_once(
            &files,
            HashMap::new(),
            HashMap::new(),
            CompilationMode::Development,
            ExportFormat::Svg,
            None,
            None,
        )
        .expect("export once");

        assert!(result.bytes.starts_with(b"<svg"));
        assert!(result.warnings.is_none());
    }

    #[test]
    fn register_template_enforces_zip_limits() {
        let files = std::fs::read("../../assets/templates/table-0.1.0.zip")
            .expect("read test template fixture");

        let err = register_template(
            &format!("limits-test-{}", Uuid::new_v4()),
            &files,
            HashMap::new(),
            HashMap::new(),
            CompilationMode::Development,
            Some(ZipLimits {
                max_entries: 1,
                ..ZipLimits::default()
            }),
        )
        .unwrap_err();

        assert!(matches!(err, FfiError::PackedTemplate(_)));
    }

    #[test]
    fn export_once_enforces_zip_limits() {
        let files = minimal_template_zip("Content");

        let err = export_once(
            &files,
            HashMap::new(),
            HashMap::new(),
            CompilationMode::Development,
            ExportFormat::Pdf,
            None,
            Some(ZipLimits {
                max_total_decompressed_bytes: 8,
                ..ZipLimits::default()
            }),
        )
        .unwrap_err();

        assert!(matches!(err, FfiError::PackedTemplate(_)));
    }
}
