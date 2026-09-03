//! The PHP integration of Oicana.
//!
//! You will want to use this through the PHP package `oicana/oicana`.

// Allow missing docs for generated PHP bindings
// Documentation is provided in the PHP wrapper package
#![allow(missing_docs)]
// Required by ext_php_rs
#![cfg_attr(windows, feature(abi_vectorcall))]

use std::collections::HashMap;

use ext_php_rs::binary::Binary;
use ext_php_rs::binary_slice::BinarySlice;
use ext_php_rs::prelude::*;
use oicana_ffi_core::panic_message;

/// Run `body`, converting any panic into a [`PhpException`].
fn catch_panic<T>(body: impl FnOnce() -> PhpResult<T>) -> PhpResult<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).unwrap_or_else(|payload| {
        Err(PhpException::default(format!(
            "internal panic: {}",
            panic_message(payload.as_ref())
        )))
    })
}

/// Compilation mode constant for production mode.
///
/// In production mode, all required inputs must be explicitly provided.
pub const COMPILATION_MODE_PRODUCTION: i64 = 0;

/// Compilation mode constant for development mode.
///
/// In development mode, default and development values from the template are used
/// when inputs are not explicitly provided.
pub const COMPILATION_MODE_DEVELOPMENT: i64 = 1;

fn compilation_mode_from_i64(mode: i64) -> oicana_ffi_core::CompilationMode {
    match mode {
        0 => oicana_ffi_core::CompilationMode::Production,
        _ => oicana_ffi_core::CompilationMode::Development,
    }
}

/// Blob input with associated metadata.
///
/// This class is used to pass binary data (images, fonts, etc.) along with
/// JSON-encoded metadata to template inputs.
#[php_class]
#[php(name = "OicanaInternal\\BlobWithMetadata")]
#[derive(Debug, Clone)]
pub struct BlobWithMetadata {
    /// The raw binary data of the blob.
    pub bytes: Vec<u8>,
    /// JSON-encoded metadata associated with the blob.
    #[php(prop)]
    pub meta: String,
}

#[php_impl]
impl BlobWithMetadata {
    /// Creates a new BlobWithMetadata instance.
    ///
    /// `bytes` is a binary-safe PHP string holding the raw blob data.
    pub fn __construct(bytes: Binary<u8>, meta: String) -> Self {
        Self {
            bytes: bytes.into(),
            meta,
        }
    }
}

/// Configure automatic cache eviction after each compilation.
///
/// # Parameters
///
/// `max_age` - Maximum age threshold, or null to disable:
///   - `null` - Disables cache eviction (cache never cleared)
///   - `0` - Clears all cache entries with every eviction
///   - `1` - Keeps only entries used since the last eviction
///   - `n` - Keeps entries used within the last n evictions
#[php_function]
#[php(name = "OicanaInternal\\configure_automatic_cache_eviction")]
pub fn configure_automatic_cache_eviction(max_age: Option<i64>) -> PhpResult<()> {
    catch_panic(|| {
        let max_age = max_age.and_then(|age| usize::try_from(age).ok());
        oicana_ffi_core::configure_automatic_cache_eviction(max_age);
        Ok(())
    })
}

/// Manually evict the comemo cache with the given age threshold.
///
/// This directly calls the underlying eviction with the specified age,
/// regardless of the configured default age.
#[php_function]
#[php(name = "OicanaInternal\\evict_cache")]
pub fn evict_cache(max_age: i64) -> PhpResult<()> {
    catch_panic(|| {
        if let Ok(max_age) = usize::try_from(max_age) {
            oicana_ffi_core::evict_cache(max_age);
        }
        Ok(())
    })
}

/// Build zip limits from optional arguments; null values keep the defaults.
fn zip_limits_from_args(
    max_entries: Option<i64>,
    max_total_decompressed_bytes: Option<i64>,
) -> PhpResult<Option<oicana_ffi_core::ZipLimits>> {
    oicana_ffi_core::ZipLimits::from_signed(max_entries, max_total_decompressed_bytes)
        .map_err(|error| PhpException::default(error.to_string()))
}

/// Register the given template. This will read the template files as a PackedTemplate and
/// compile it once with the given inputs. The Typst World will be cached and reused for
/// subsequent calls to the other methods with the same template identifier.
#[php_function]
#[php(name = "OicanaInternal\\register_template")]
pub fn register_template(
    template: String,
    files: BinarySlice<u8>,
    json_inputs: HashMap<String, String>,
    blob_inputs: HashMap<String, &BlobWithMetadata>,
    compilation_mode: i64,
    max_entries: Option<i64>,
    max_total_decompressed_bytes: Option<i64>,
) -> PhpResult<String> {
    catch_panic(|| {
        oicana_ffi_core::register_template(
            &template,
            *files,
            json_inputs,
            into_core_blobs(blob_inputs),
            compilation_mode_from_i64(compilation_mode),
            zip_limits_from_args(max_entries, max_total_decompressed_bytes)?,
        )
        .map_err(into_php_err)
    })
}

/// Result of a one-shot export: document bytes plus any compilation warnings.
#[php_class]
#[php(name = "OicanaInternal\\ExportOnceResult")]
pub struct ExportOnceResult {
    document: Vec<u8>,
    /// Compilation warnings, or null if there were none.
    #[php(prop)]
    pub warnings: Option<String>,
}

#[php_impl]
impl ExportOnceResult {
    /// The exported document as a binary string.
    pub fn document(&self) -> Binary<u8> {
        self.document.clone().into()
    }
}

/// Compile and export the given template once, without caching anything.
///
/// `page_range` is a JSON object `{ "start"?: int, "end"?: int }` with 0-based,
/// inclusive bounds. If not set, the whole document is exported.
#[php_function]
#[php(name = "OicanaInternal\\export_template_once")]
#[allow(clippy::too_many_arguments)]
pub fn export_template_once(
    files: BinarySlice<u8>,
    json_inputs: HashMap<String, String>,
    blob_inputs: HashMap<String, &BlobWithMetadata>,
    compilation_mode: i64,
    export_format: String,
    page_range: Option<String>,
    max_entries: Option<i64>,
    max_total_decompressed_bytes: Option<i64>,
) -> PhpResult<ExportOnceResult> {
    catch_panic(|| {
        let format = oicana_ffi_core::parse_export_format(&export_format).map_err(into_php_err)?;
        let page =
            oicana_ffi_core::parse_page_range(page_range.as_deref()).map_err(into_php_err)?;
        let result = oicana_ffi_core::export_once(
            *files,
            json_inputs,
            into_core_blobs(blob_inputs),
            compilation_mode_from_i64(compilation_mode),
            format,
            page,
            zip_limits_from_args(max_entries, max_total_decompressed_bytes)?,
        )
        .map_err(into_php_err)?;
        Ok(ExportOnceResult {
            document: result.bytes,
            warnings: result.warnings,
        })
    })
}

/// Configure the coloring of compilation diagnostics like warnings and errors.
#[php_function]
#[php(name = "OicanaInternal\\configure_diagnostic_color")]
pub fn configure_diagnostic_color(ansi: bool) -> PhpResult<()> {
    catch_panic(|| {
        let color = if ansi {
            oicana_ffi_core::DiagnosticColor::Ansi
        } else {
            oicana_ffi_core::DiagnosticColor::None
        };
        oicana_ffi_core::configure_diagnostic_color(color);
        Ok(())
    })
}

/// Register a single font from its raw file content.
///
/// Returns the number of font faces that were added, so `0` means the data held
/// no font Typst can read.
#[php_function]
#[php(name = "OicanaInternal\\register_font")]
pub fn register_font(font: BinarySlice<u8>) -> PhpResult<i64> {
    catch_panic(|| Ok(oicana_ffi_core::register_font(font.to_vec()) as i64))
}

/// Register fonts from files on disk, not retaining their data until it is used.
///
/// Returns the number of font faces that were added.
#[php_function]
#[php(name = "OicanaInternal\\register_font_paths")]
pub fn register_font_paths(paths: Vec<String>) -> PhpResult<i64> {
    catch_panic(|| {
        let paths = paths.into_iter().map(std::path::PathBuf::from).collect();
        Ok(oicana_ffi_core::register_font_paths(paths) as i64)
    })
}

/// All font faces currently registered by the host, as a JSON array of
/// `{ "family": ..., "path": ... }` objects.
#[php_function]
#[php(name = "OicanaInternal\\registered_fonts")]
pub fn registered_fonts() -> PhpResult<String> {
    catch_panic(|| {
        serde_json::to_string(&oicana_ffi_core::registered_fonts()).map_err(|error| {
            PhpException::default(format!("Failed to serialize registered fonts: {error}"))
        })
    })
}

/// Drop all fonts registered by the host.
///
/// Templates that are already registered keep the fonts they were created with.
#[php_function]
#[php(name = "OicanaInternal\\clear_fonts")]
pub fn clear_fonts() -> PhpResult<()> {
    catch_panic(|| {
        oicana_ffi_core::clear_fonts();
        Ok(())
    })
}

/// Compile the identified template with the given inputs.
///
/// Calling this method requires a previous call to `register_template` with the same template
/// identifier.
#[php_function]
#[php(name = "OicanaInternal\\compile_template")]
pub fn compile_template(
    template: String,
    json_inputs: HashMap<String, String>,
    blob_inputs: HashMap<String, &BlobWithMetadata>,
    compilation_mode: i64,
) -> PhpResult<String> {
    catch_panic(|| {
        oicana_ffi_core::compile_template(
            &template,
            json_inputs,
            into_core_blobs(blob_inputs),
            compilation_mode_from_i64(compilation_mode),
        )
        .map_err(into_php_err)
    })
}

/// Load the manifest of the given template.
///
/// Calling this method requires a previous call to `register_template` with the same template
/// identifier.
#[php_function]
#[php(name = "OicanaInternal\\manifest")]
pub fn manifest(template: String) -> PhpResult<String> {
    catch_panic(|| oicana_ffi_core::manifest(&template).map_err(into_php_err))
}

/// Return the sizes (in points) of every page of a compiled document as a JSON
/// array of `{ "width": float, "height": float }`.
#[php_function]
#[php(name = "OicanaInternal\\document_pages")]
pub fn document_pages(document_id: String) -> PhpResult<String> {
    catch_panic(|| oicana_ffi_core::document_pages(&document_id).map_err(into_php_err))
}

/// Load the source of the given file in the template.
///
/// Calling this method requires a previous call to `register_template` with the same template
/// identifier.
#[php_function]
#[php(name = "OicanaInternal\\get_source")]
pub fn get_source(template: String, file: String) -> PhpResult<String> {
    catch_panic(|| oicana_ffi_core::get_source(&template, &file).map_err(into_php_err))
}

/// Load the binary file content from the template.
///
/// Calling this method requires a previous call to `register_template` with the same template
/// identifier.
#[php_function]
#[php(name = "OicanaInternal\\get_file")]
pub fn get_file(template: String, file: String) -> PhpResult<Binary<u8>> {
    catch_panic(|| {
        oicana_ffi_core::get_file(&template, &file)
            .map(Binary::from)
            .map_err(into_php_err)
    })
}

/// Export the given document
///
/// `page_range` is a JSON object `{ "start"?: int, "end"?: int }` with 0-based,
/// inclusive bounds. If not set, the whole document is exported.
///
/// Make sure to call `remove_document` with the document_id afterwards, to free the memory.
#[php_function]
#[php(name = "OicanaInternal\\export_document")]
pub fn export_document(
    document_id: String,
    export_format: String,
    page_range: Option<String>,
) -> PhpResult<Binary<u8>> {
    catch_panic(|| {
        let format = oicana_ffi_core::parse_export_format(&export_format).map_err(into_php_err)?;
        let page =
            oicana_ffi_core::parse_page_range(page_range.as_deref()).map_err(into_php_err)?;
        oicana_ffi_core::export_document(&document_id, format, page)
            .map(Binary::from)
            .map_err(into_php_err)
    })
}

/// Remove the document from the cache.
#[php_function]
#[php(name = "OicanaInternal\\remove_document")]
pub fn remove_document(document_id: String) -> PhpResult<()> {
    catch_panic(|| {
        oicana_ffi_core::remove_document(&document_id);
        Ok(())
    })
}

/// Return any compilation warnings produced for the given document, or `null`
/// if there were none. Warnings are cleared when the document is removed.
#[php_function]
#[php(name = "OicanaInternal\\get_warnings")]
pub fn get_warnings(document_id: String) -> PhpResult<Option<String>> {
    catch_panic(|| Ok(oicana_ffi_core::get_warnings(&document_id)))
}

/// Enable or disable JSON schema validation for the given template.
///
/// When enabled (the default), JSON inputs are validated against their schemas
/// before compilation.
#[php_function]
#[php(name = "OicanaInternal\\set_validate_inputs")]
pub fn set_validate_inputs(template: String, validate: bool) -> PhpResult<()> {
    catch_panic(|| oicana_ffi_core::set_validate_inputs(&template, validate).map_err(into_php_err))
}

/// Remove the world from the cache.
///
/// The template will have to be registered again before it can be compiled again.
#[php_function]
#[php(name = "OicanaInternal\\remove_world")]
pub fn remove_world(template_id: String) -> PhpResult<()> {
    catch_panic(|| {
        oicana_ffi_core::remove_world(&template_id);
        Ok(())
    })
}

fn into_core_blobs(
    blobs: HashMap<String, &BlobWithMetadata>,
) -> HashMap<String, oicana_ffi_core::BlobWithMetadata> {
    blobs
        .into_iter()
        .map(|(key, value)| {
            (
                key,
                oicana_ffi_core::BlobWithMetadata {
                    bytes: value.bytes.clone(),
                    meta: value.meta.clone(),
                },
            )
        })
        .collect()
}

fn into_php_err(error: oicana_ffi_core::FfiError) -> PhpException {
    PhpException::default(error.to_string())
}

/// Registers the PHP module with ext-php-rs.
#[php_module]
#[php(startup = startup_function)]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        .name("oicana")
        .function(wrap_function!(configure_automatic_cache_eviction))
        .function(wrap_function!(evict_cache))
        .function(wrap_function!(register_template))
        .function(wrap_function!(export_template_once))
        .function(wrap_function!(configure_diagnostic_color))
        .function(wrap_function!(compile_template))
        .function(wrap_function!(manifest))
        .function(wrap_function!(document_pages))
        .function(wrap_function!(get_source))
        .function(wrap_function!(get_file))
        .function(wrap_function!(export_document))
        .function(wrap_function!(remove_document))
        .function(wrap_function!(get_warnings))
        .function(wrap_function!(remove_world))
        .function(wrap_function!(set_validate_inputs))
        .function(wrap_function!(register_font))
        .function(wrap_function!(register_font_paths))
        .function(wrap_function!(registered_fonts))
        .function(wrap_function!(clear_fonts))
        .class::<BlobWithMetadata>()
        .class::<ExportOnceResult>()
}

fn startup_function(_ty: i32, _mod_num: i32) -> i32 {
    0
}
