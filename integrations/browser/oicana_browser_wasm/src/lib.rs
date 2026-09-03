//! Lower level WASM bindings for Oicana.
//!
//! You most likely want to use the npm package `@oicana/browser` instead.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};

use js_sys::Uint8Array;
use log::{trace, warn, Level, LevelFilter};
use serde::Deserialize;
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

use oicana_world::get_current_time;

/// Configure automatic cache eviction after each compilation.
///
/// # Parameters
///
/// `max_age` (start value: 10) - Maximum age threshold, or null to disable:
///   - `null` - Disables cache eviction (cache never cleared)
///   - `0` - Clears all cache entries with every eviction
///   - `1` - Keeps only entries used since the last eviction
///   - `n` - Keeps entries used within the last n evictions
#[wasm_bindgen]
pub fn configure_automatic_cache_eviction(max_age: Option<usize>) {
    oicana_ffi_core::configure_automatic_cache_eviction(max_age);
}

/// Manually evict the comemo cache with the given age threshold.
///
/// This directly calls the underlying eviction with the specified age,
/// regardless of the configured default age.
#[wasm_bindgen]
pub fn evict_cache(max_age: usize) {
    oicana_ffi_core::evict_cache(max_age);
}

/// Register the given template. This will read the template as a [`PackedTemplate`] and compile it
/// once with the given inputs. The Typst [`typst::World`] will be cached and reused for subsequent
/// calls to the other methods with the same template identifier.
#[wasm_bindgen]
pub fn register_template(
    template: String,
    files: &Uint8Array,
    json_inputs: JsValue,
    blob_inputs: JsValue,
    compilation_mode: JsValue,
    limits: JsValue,
) -> Result<String, String> {
    init_logging();
    let start = get_current_time();

    let json_map = decode_json_inputs(json_inputs)?;
    let blob_map = decode_blob_inputs(blob_inputs)?;
    let compilation_mode: CompilationMode = from_value(compilation_mode)
        .map_err(|error| format!("Failed to convert to compilation mode: {error:?}"))?;
    let limits = decode_zip_limits(limits)?;

    let mut bytes = vec![0; files.length() as usize];
    files.copy_to(&mut bytes[..]);

    let result_id = oicana_ffi_core::register_template(
        &template,
        &bytes,
        json_map,
        blob_map,
        compilation_mode.into(),
        limits,
    )
    .map_err(|error| error.to_string())?;

    log_warnings(&result_id);
    trace!(
        "Done compiling document in {}ms",
        get_current_time() - start
    );
    Ok(result_id)
}

/// Result of a one-shot export.
#[wasm_bindgen(getter_with_clone)]
pub struct ExportOnceResult {
    /// The exported document.
    pub data: Vec<u8>,
    /// Compilation warnings, if any.
    pub warnings: Option<String>,
}

/// Compile and export the given template once, without keeping the template in the cache.
///
/// `page_range` is a `{ start?: number; end?: number }` object with 0-based,
/// inclusive bounds. If not set, the whole document is exported.
#[wasm_bindgen]
pub fn export_template_once(
    files: &Uint8Array,
    json_inputs: JsValue,
    blob_inputs: JsValue,
    compilation_mode: JsValue,
    export_format: JsValue,
    page_range: JsValue,
    limits: JsValue,
) -> Result<ExportOnceResult, String> {
    init_logging();
    let start = get_current_time();

    let json_map = decode_json_inputs(json_inputs)?;
    let blob_map = decode_blob_inputs(blob_inputs)?;
    let compilation_mode: CompilationMode = from_value(compilation_mode)
        .map_err(|error| format!("Failed to convert to compilation mode: {error:?}"))?;
    let format: oicana_ffi_core::ExportFormat = from_value(export_format)
        .map_err(|error| format!("Failed to convert to export format: {error:?}"))?;
    let page: Option<oicana_ffi_core::PageRange> = from_value(page_range)
        .map_err(|error| format!("Failed to convert to page range: {error:?}"))?;
    let limits = decode_zip_limits(limits)?;

    let mut bytes = vec![0; files.length() as usize];
    files.copy_to(&mut bytes[..]);

    let result = oicana_ffi_core::export_once(
        &bytes,
        json_map,
        blob_map,
        compilation_mode.into(),
        format,
        page,
        limits,
    )
    .map_err(|error| error.to_string())?;

    trace!(
        "Done exporting document in {}ms",
        get_current_time() - start
    );
    Ok(ExportOnceResult {
        data: result.bytes,
        warnings: result.warnings,
    })
}

/// Make fonts available to every template registered from now on.
///
/// Data that holds no font Typst can read is ignored. Returns the number of font
/// faces that were added.
#[wasm_bindgen]
pub fn register_fonts(fonts: Vec<Uint8Array>) -> usize {
    let fonts = fonts.into_iter().map(|font| font.to_vec()).collect();
    oicana_ffi_core::register_fonts(fonts)
}

/// All font faces currently registered, as `{ family, path }` objects.
#[wasm_bindgen]
pub fn registered_fonts() -> Result<JsValue, String> {
    let fonts = oicana_ffi_core::registered_fonts();
    to_value(&fonts).map_err(|error| format!("Failed to serialize registered fonts: {error}"))
}

/// Drop all fonts registered by the host.
///
/// Templates that are already registered keep the fonts they were created with.
#[wasm_bindgen]
pub fn clear_fonts() {
    oicana_ffi_core::clear_fonts();
}

/// Configure the coloring of compilation diagnostics.
#[wasm_bindgen]
pub fn configure_diagnostic_color(ansi: bool) {
    let color = if ansi {
        oicana_ffi_core::DiagnosticColor::Ansi
    } else {
        oicana_ffi_core::DiagnosticColor::None
    };
    oicana_ffi_core::configure_diagnostic_color(color);
}

/// Compile the identified template with the given inputs.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[wasm_bindgen]
pub fn compile_template(
    template: String,
    json_inputs: JsValue,
    blob_inputs: JsValue,
    compilation_mode: JsValue,
) -> Result<String, String> {
    init_logging();
    let start = get_current_time();

    let json_map = decode_json_inputs(json_inputs)?;
    let blob_map = decode_blob_inputs(blob_inputs)?;
    let compilation_mode: CompilationMode = from_value(compilation_mode)
        .map_err(|error| format!("Failed to convert to compilation mode: {error:?}"))?;

    let result_id =
        oicana_ffi_core::compile_template(&template, json_map, blob_map, compilation_mode.into())
            .map_err(|error| error.to_string())?;

    log_warnings(&result_id);
    trace!(
        "Done preparing document in {}ms",
        get_current_time() - start
    );
    Ok(result_id)
}

/// Load the manifest of the given template.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[wasm_bindgen]
pub fn manifest(template: String) -> Result<String, String> {
    init_logging();
    oicana_ffi_core::manifest(&template).map_err(|error| error.to_string())
}

/// Return the sizes (in points) of every page of a compiled document as a JSON
/// array of `{ "width": number, "height": number }`.
///
/// Calling this method requires a previous call to [`compile_template`] (or
/// [`register_template`]) that produced the given `document_id`.
#[wasm_bindgen]
pub fn document_pages(document_id: String) -> Result<String, String> {
    oicana_ffi_core::document_pages(&document_id).map_err(|error| error.to_string())
}

/// Load the source of the given file in the template.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[wasm_bindgen]
pub fn get_source(template: String, file: String) -> Result<String, String> {
    oicana_ffi_core::get_source(&template, &file).map_err(|error| error.to_string())
}

/// Load the source of the given file in the template.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[wasm_bindgen]
pub fn get_file(template: String, file: String) -> Result<Uint8Array, String> {
    let bytes = oicana_ffi_core::get_file(&template, &file).map_err(|error| error.to_string())?;
    Ok(bytes_to_js_array(&bytes))
}

/// Remove the document from the cache.
#[wasm_bindgen]
pub fn remove_document(document_id: String) -> Result<(), String> {
    oicana_ffi_core::remove_document(&document_id);
    Ok(())
}

/// Return any compilation warnings produced for the given document, or
/// `undefined` if there were none. Warnings are cleared when the document
/// is removed.
#[wasm_bindgen]
pub fn get_warnings(document_id: String) -> Option<String> {
    oicana_ffi_core::get_warnings(&document_id)
}

/// Enable or disable JSON schema validation for the given template.
///
/// When enabled (the default), JSON inputs are validated against their schemas
/// before compilation.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[wasm_bindgen]
pub fn set_validate_inputs(template: String, validate: bool) -> Result<(), String> {
    oicana_ffi_core::set_validate_inputs(&template, validate).map_err(|error| error.to_string())
}

/// Remove the world from the cache.
///
/// The template will have to be registered again before it can be compiled again.
#[wasm_bindgen]
pub fn remove_world(template_id: String) -> Result<(), String> {
    oicana_ffi_core::remove_world(&template_id);
    Ok(())
}

/// Export the given document
///
/// `page_range` is a `{ start?: number; end?: number }` object with 0-based,
/// inclusive bounds. If not set, the whole document is exported.
///
/// Make sure to call `removeDocument` with the documentId afterwards, to free the memory.
#[wasm_bindgen]
pub fn export_document(
    document_id: String,
    export_format: JsValue,
    page_range: JsValue,
) -> Result<Uint8Array, String> {
    let format: oicana_ffi_core::ExportFormat = from_value(export_format)
        .map_err(|error| format!("Failed to convert to export format: {error:?}"))?;
    let page: Option<oicana_ffi_core::PageRange> = from_value(page_range)
        .map_err(|error| format!("Failed to convert to page range: {error:?}"))?;
    let start = get_current_time();
    let bytes = oicana_ffi_core::export_document(&document_id, format, page)
        .map_err(|error| error.to_string())?;
    trace!("Exported document in {}ms", get_current_time() - start);
    Ok(bytes_to_js_array(&bytes))
}

/// The configured maximum log level as a [`LevelFilter`] discriminant.
/// Defaults to `Warn`.
static LOG_LEVEL: AtomicU8 = AtomicU8::new(LevelFilter::Warn as u8);

/// Set the maximum log level for console output.
///
/// Accepts `off`, `error`, `warn`, `info`, `debug`, or `trace`. The default is `warn`.
#[wasm_bindgen]
pub fn set_log_level(level: &str) -> Result<(), String> {
    let filter = match level.to_ascii_lowercase().as_str() {
        "off" => LevelFilter::Off,
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        other => return Err(format!("Unknown log level '{other}'")),
    };
    LOG_LEVEL.store(filter as u8, Ordering::Relaxed);
    log::set_max_level(filter);
    Ok(())
}

fn configured_log_level() -> LevelFilter {
    match LOG_LEVEL.load(Ordering::Relaxed) {
        0 => LevelFilter::Off,
        1 => LevelFilter::Error,
        2 => LevelFilter::Warn,
        3 => LevelFilter::Info,
        4 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    }
}

fn init_logging() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(Level::Trace);
    log::set_max_level(configured_log_level());
}

fn log_warnings(document_id: &str) {
    if let Some(warnings) = oicana_ffi_core::get_warnings(document_id) {
        warn!("{warnings}");
    }
}

fn bytes_to_js_array(bytes: &[u8]) -> Uint8Array {
    let uint8_array = Uint8Array::new_with_length(bytes.len() as u32);
    uint8_array.copy_from(bytes);
    uint8_array
}

fn decode_json_inputs(value: JsValue) -> Result<HashMap<String, String>, String> {
    from_value(value).map_err(|error| {
        format!("Failed to deserialize HashMap<String, String> from JavaScript value: {error:?}")
    })
}

fn decode_blob_inputs(
    value: JsValue,
) -> Result<HashMap<String, oicana_ffi_core::BlobWithMetadata>, String> {
    let blobs: HashMap<String, BlobInput> = from_value(value).map_err(|error| {
        format!("Failed to deserialize HashMap<String, BlobInput> from JavaScript value: {error:?}")
    })?;
    blobs
        .into_iter()
        .map(|(key, blob)| {
            let meta = match blob.metadata {
                Some(metadata) => serde_json::to_string(&metadata)
                    .map_err(|error| format!("Failed to encode metadata for '{key}': {error:?}"))?,
                None => "{}".to_owned(),
            };
            Ok((
                key,
                oicana_ffi_core::BlobWithMetadata {
                    bytes: blob.data,
                    meta,
                },
            ))
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZipLimits {
    max_entries: Option<u64>,
    max_total_decompressed_bytes: Option<u64>,
}

fn decode_zip_limits(value: JsValue) -> Result<Option<oicana_ffi_core::ZipLimits>, String> {
    let limits: Option<ZipLimits> = from_value(value)
        .map_err(|error| format!("Failed to deserialize zip limits: {error:?}"))?;
    let Some(limits) = limits else {
        return Ok(None);
    };
    Ok(oicana_ffi_core::ZipLimits::from_optional(
        limits.max_entries,
        limits.max_total_decompressed_bytes,
    ))
}

#[derive(Deserialize)]
enum CompilationMode {
    #[serde(alias = "production")]
    Production,
    #[serde(alias = "development")]
    Development,
}

impl From<CompilationMode> for oicana_ffi_core::CompilationMode {
    fn from(value: CompilationMode) -> Self {
        match value {
            CompilationMode::Development => oicana_ffi_core::CompilationMode::Development,
            CompilationMode::Production => oicana_ffi_core::CompilationMode::Production,
        }
    }
}

#[derive(Deserialize)]
struct BlobInput {
    data: Vec<u8>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}
