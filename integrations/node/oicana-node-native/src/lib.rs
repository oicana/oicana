//! The Node.js integration of Oicana.
//!
//! You will want to use this through the npm package `@oicana/node`.

#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use std::collections::HashMap;

use napi::bindgen_prelude::{AsyncTask, Buffer, Result, Uint8Array};
use napi::{Env, Error, Task};

/// Configure automatic cache eviction after each compilation.
///
/// # Parameters
///
/// `max_age` (start value: 10) - Maximum age threshold, or null to disable:
///   - `undefined`/`null` - Disables cache eviction (cache never cleared)
///   - `0` - Clears all cache entries with every eviction
///   - `1` - Keeps only entries used since the last eviction
///   - `n` - Keeps entries used within the last n evictions
#[napi(catch_unwind)]
pub fn configure_automatic_cache_eviction(max_age: Option<u32>) {
  oicana_ffi_core::configure_automatic_cache_eviction(max_age.map(|v| v as usize));
}

/// Manually evict the comemo cache with the given age threshold.
///
/// This directly calls the underlying eviction with the specified age,
/// regardless of the configured default age.
#[napi(catch_unwind)]
pub fn evict_cache(max_age: u32) {
  oicana_ffi_core::evict_cache(max_age as usize);
}

/// Register the given template. This will read the template files as a [`PackedTemplate`] and
/// compile it once with the given inputs. The Typst [`typst::World`] will be cached and reused for
/// subsequent calls to the other methods with the same template identifier.
#[napi(catch_unwind)]
pub fn register_template(
  template: String,
  files: Uint8Array,
  json_inputs: HashMap<String, String>,
  blob_inputs: HashMap<String, BlobWithMetadata>,
  compilation_mode: CompilationMode,
  limits: Option<ZipLimits>,
) -> Result<String> {
  oicana_ffi_core::register_template(
    &template,
    &files,
    json_inputs,
    into_core_blobs(blob_inputs),
    compilation_mode.into(),
    into_core_limits(limits)?,
  )
  .map_err(into_napi_err)
}

/// Compile the identified template with the given inputs.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[napi(catch_unwind)]
pub fn compile_template(
  template: String,
  json_inputs: HashMap<String, String>,
  blob_inputs: HashMap<String, BlobWithMetadata>,
  compilation_mode: CompilationMode,
) -> Result<String> {
  oicana_ffi_core::compile_template(
    &template,
    json_inputs,
    into_core_blobs(blob_inputs),
    compilation_mode.into(),
  )
  .map_err(into_napi_err)
}

/// Background task registering a template on the libuv thread pool.
pub struct RegisterTemplateTask {
  template: String,
  files: Vec<u8>,
  json_inputs: HashMap<String, String>,
  blob_inputs: HashMap<String, oicana_ffi_core::BlobWithMetadata>,
  compilation_mode: oicana_ffi_core::CompilationMode,
  limits: Option<oicana_ffi_core::ZipLimits>,
}

impl Task for RegisterTemplateTask {
  type Output = String;
  type JsValue = String;

  fn compute(&mut self) -> Result<Self::Output> {
    catch_panic(|| {
      oicana_ffi_core::register_template(
        &self.template,
        &std::mem::take(&mut self.files),
        std::mem::take(&mut self.json_inputs),
        std::mem::take(&mut self.blob_inputs),
        self.compilation_mode,
        self.limits,
      )
      .map_err(into_napi_err)
    })
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

/// Register the given template on a background thread.
///
/// The returned promise resolves to the document id of the initial warm-up
/// compilation. Unlike [`register_template`], this does not block the Node.js
/// event loop while the template is read and compiled.
#[napi(catch_unwind, ts_return_type = "Promise<string>")]
pub fn register_template_async(
  template: String,
  files: Uint8Array,
  json_inputs: HashMap<String, String>,
  blob_inputs: HashMap<String, BlobWithMetadata>,
  compilation_mode: CompilationMode,
  limits: Option<ZipLimits>,
) -> Result<AsyncTask<RegisterTemplateTask>> {
  Ok(AsyncTask::new(RegisterTemplateTask {
    template,
    files: files.to_vec(),
    json_inputs,
    blob_inputs: into_core_blobs(blob_inputs),
    compilation_mode: compilation_mode.into(),
    limits: into_core_limits(limits)?,
  }))
}

/// Result of a one-shot export.
#[napi(object)]
pub struct ExportOnceResult {
  /// The exported document.
  pub data: Buffer,
  /// Compilation warnings, if any.
  pub warnings: Option<String>,
}

/// Compile and export the given template once, without caching the template or document.
///
/// `page_range` is a JSON object `{ "start"?: number, "end"?: number }` with
/// 0-based, inclusive bounds. If not set, the whole document is exported.
#[napi(catch_unwind)]
pub fn export_template_once(
  files: Uint8Array,
  json_inputs: HashMap<String, String>,
  blob_inputs: HashMap<String, BlobWithMetadata>,
  compilation_mode: CompilationMode,
  export_format: String,
  page_range: Option<String>,
  limits: Option<ZipLimits>,
) -> Result<ExportOnceResult> {
  let format = oicana_ffi_core::parse_export_format(&export_format).map_err(into_napi_err)?;
  let page = oicana_ffi_core::parse_page_range(page_range.as_deref()).map_err(into_napi_err)?;
  let result = oicana_ffi_core::export_once(
    &files,
    json_inputs,
    into_core_blobs(blob_inputs),
    compilation_mode.into(),
    format,
    page,
    into_core_limits(limits)?,
  )
  .map_err(into_napi_err)?;
  Ok(ExportOnceResult {
    data: result.bytes.into(),
    warnings: result.warnings,
  })
}

/// Background task compiling and exporting a template once on the libuv thread pool.
pub struct ExportTemplateOnceTask {
  files: Vec<u8>,
  json_inputs: HashMap<String, String>,
  blob_inputs: HashMap<String, oicana_ffi_core::BlobWithMetadata>,
  compilation_mode: oicana_ffi_core::CompilationMode,
  export_format: String,
  page_range: Option<String>,
  limits: Option<oicana_ffi_core::ZipLimits>,
}

impl Task for ExportTemplateOnceTask {
  type Output = oicana_ffi_core::ExportOnceResult;
  type JsValue = ExportOnceResult;

  fn compute(&mut self) -> Result<Self::Output> {
    catch_panic(|| {
      let format =
        oicana_ffi_core::parse_export_format(&self.export_format).map_err(into_napi_err)?;
      let pages =
        oicana_ffi_core::parse_page_range(self.page_range.as_deref()).map_err(into_napi_err)?;
      oicana_ffi_core::export_once(
        &std::mem::take(&mut self.files),
        std::mem::take(&mut self.json_inputs),
        std::mem::take(&mut self.blob_inputs),
        self.compilation_mode,
        format,
        pages,
        self.limits,
      )
      .map_err(into_napi_err)
    })
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(ExportOnceResult {
      data: output.bytes.into(),
      warnings: output.warnings,
    })
  }
}

/// Compile and export the given template once on a background thread.
///
/// The returned promise resolves to the exported bytes and any compilation
/// warnings. Unlike [`export_template_once`], this does not block the Node.js
/// event loop.
#[napi(catch_unwind, ts_return_type = "Promise<ExportOnceResult>")]
pub fn export_template_once_async(
  files: Uint8Array,
  json_inputs: HashMap<String, String>,
  blob_inputs: HashMap<String, BlobWithMetadata>,
  compilation_mode: CompilationMode,
  export_format: String,
  page_range: Option<String>,
  limits: Option<ZipLimits>,
) -> Result<AsyncTask<ExportTemplateOnceTask>> {
  Ok(AsyncTask::new(ExportTemplateOnceTask {
    files: files.to_vec(),
    json_inputs,
    blob_inputs: into_core_blobs(blob_inputs),
    compilation_mode: compilation_mode.into(),
    export_format,
    page_range,
    limits: into_core_limits(limits)?,
  }))
}

/// Background task compiling a template on the libuv thread pool.
pub struct CompileTemplateTask {
  template: String,
  json_inputs: HashMap<String, String>,
  blob_inputs: HashMap<String, oicana_ffi_core::BlobWithMetadata>,
  compilation_mode: oicana_ffi_core::CompilationMode,
}

impl Task for CompileTemplateTask {
  type Output = String;
  type JsValue = String;

  fn compute(&mut self) -> Result<Self::Output> {
    catch_panic(|| {
      oicana_ffi_core::compile_template(
        &self.template,
        std::mem::take(&mut self.json_inputs),
        std::mem::take(&mut self.blob_inputs),
        self.compilation_mode,
      )
      .map_err(into_napi_err)
    })
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

/// Compile the identified template with the given inputs on a background thread.
///
/// The returned promise resolves to the document id. Unlike [`compile_template`],
/// this does not block the Node.js event loop while the compilation runs.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[napi(catch_unwind, ts_return_type = "Promise<string>")]
pub fn compile_template_async(
  template: String,
  json_inputs: HashMap<String, String>,
  blob_inputs: HashMap<String, BlobWithMetadata>,
  compilation_mode: CompilationMode,
) -> AsyncTask<CompileTemplateTask> {
  AsyncTask::new(CompileTemplateTask {
    template,
    json_inputs,
    blob_inputs: into_core_blobs(blob_inputs),
    compilation_mode: compilation_mode.into(),
  })
}

/// Load the manifest of the given template.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[napi(catch_unwind)]
pub fn manifest(template: String) -> Result<String> {
  oicana_ffi_core::manifest(&template).map_err(into_napi_err)
}

/// Return the sizes (in points) of every page of a compiled document as a JSON
/// array of `{ "width": number, "height": number }`.
#[napi(catch_unwind)]
pub fn document_pages(document_id: String) -> Result<String> {
  oicana_ffi_core::document_pages(&document_id).map_err(into_napi_err)
}

/// Load the source of the given file in the template.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[napi(catch_unwind)]
pub fn get_source(template: String, file: String) -> Result<String> {
  oicana_ffi_core::get_source(&template, &file).map_err(into_napi_err)
}

/// Load the source of the given file in the template.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[napi(catch_unwind)]
pub fn get_file(template: String, file: String) -> Result<Buffer> {
  oicana_ffi_core::get_file(&template, &file)
    .map(Into::into)
    .map_err(into_napi_err)
}

/// Export the given document
///
/// `page_range` is a JSON object `{ "start"?: number, "end"?: number }` with
/// 0-based, inclusive bounds. If not set, the whole document is exported.
///
/// Make sure to call `removeDocument` with the documentId afterwards, to free the memory.
#[napi(catch_unwind)]
pub fn export_document(
  document_id: String,
  export_format: String,
  page_range: Option<String>,
) -> Result<Buffer> {
  let format = oicana_ffi_core::parse_export_format(&export_format).map_err(into_napi_err)?;
  let page = oicana_ffi_core::parse_page_range(page_range.as_deref()).map_err(into_napi_err)?;
  oicana_ffi_core::export_document(&document_id, format, page)
    .map(Into::into)
    .map_err(into_napi_err)
}

/// Background task exporting a compiled document on the libuv thread pool.
pub struct ExportDocumentTask {
  document_id: String,
  export_format: String,
  page_range: Option<String>,
}

impl Task for ExportDocumentTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    catch_panic(|| {
      let format =
        oicana_ffi_core::parse_export_format(&self.export_format).map_err(into_napi_err)?;
      let pages =
        oicana_ffi_core::parse_page_range(self.page_range.as_deref()).map_err(into_napi_err)?;
      oicana_ffi_core::export_document(&self.document_id, format, pages).map_err(into_napi_err)
    })
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

/// Export the given document on a background thread.
///
/// The returned promise resolves to the exported bytes. Unlike [`export_document`],
/// this does not block the Node.js event loop while the export runs.
///
/// `page_range` is a JSON object `{ "start"?: number, "end"?: number }` with
/// 0-based, inclusive bounds. If not set, the whole document is exported.
///
/// Make sure to call `removeDocument` with the documentId afterwards, to free the memory.
#[napi(catch_unwind, ts_return_type = "Promise<Buffer>")]
pub fn export_document_async(
  document_id: String,
  export_format: String,
  page_range: Option<String>,
) -> AsyncTask<ExportDocumentTask> {
  AsyncTask::new(ExportDocumentTask {
    document_id,
    export_format,
    page_range,
  })
}

/// Remove the document from the cache.
#[napi(catch_unwind)]
pub fn remove_document(document_id: String) -> Result<()> {
  oicana_ffi_core::remove_document(&document_id);
  Ok(())
}

/// Return any compilation warnings produced for the given document, or `null`
/// if there were none. Warnings are cleared when the document is removed.
#[napi(catch_unwind)]
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
#[napi(catch_unwind)]
pub fn set_validate_inputs(template: String, validate: bool) -> Result<()> {
  oicana_ffi_core::set_validate_inputs(&template, validate).map_err(into_napi_err)
}

/// Remove the world from the cache.
///
/// The template will have to be registered again before it can be compiled again.
#[napi(catch_unwind)]
pub fn remove_world(template_id: String) -> Result<()> {
  oicana_ffi_core::remove_world(&template_id);
  Ok(())
}

#[napi]
pub enum CompilationMode {
  Production,
  Development,
}

/// Color mode for compilation diagnostics.
#[napi]
pub enum DiagnosticColor {
  None,
  Ansi,
}

/// Configure the coloring of compilation diagnostics like warnings and errors.
#[napi(catch_unwind)]
pub fn configure_diagnostic_color(color: DiagnosticColor) {
  let color = match color {
    DiagnosticColor::Ansi => oicana_ffi_core::DiagnosticColor::Ansi,
    DiagnosticColor::None => oicana_ffi_core::DiagnosticColor::None,
  };
  oicana_ffi_core::configure_diagnostic_color(color);
}

/// A font face made available to templates by the host.
#[napi(object)]
pub struct RegisteredFont {
  /// The family name, as used in Typst's `text(font: ...)`.
  pub family: String,
  /// The file the face was read from; absent for fonts registered from memory.
  pub path: Option<String>,
}

/// Make fonts available to every template registered from now on.
///
/// Data that holds no font Typst can read is ignored. Returns the number of font
/// faces that were added.
#[napi(catch_unwind)]
pub fn register_fonts(fonts: Vec<Uint8Array>) -> u32 {
  let fonts = fonts.into_iter().map(|font| font.to_vec()).collect();
  oicana_ffi_core::register_fonts(fonts) as u32
}

/// Make fonts on disk available to every template registered from now on.
///
/// Returns the number of font faces that were added.
#[napi(catch_unwind)]
pub fn register_font_paths(paths: Vec<String>) -> u32 {
  let paths = paths.into_iter().map(std::path::PathBuf::from).collect();
  oicana_ffi_core::register_font_paths(paths) as u32
}

/// All font faces currently registered by the host.
#[napi(catch_unwind)]
pub fn registered_fonts() -> Vec<RegisteredFont> {
  oicana_ffi_core::registered_fonts()
    .into_iter()
    .map(|font| RegisteredFont {
      family: font.family,
      path: font.path,
    })
    .collect()
}

/// Drop all fonts registered by the host.
///
/// Templates that are already registered keep the fonts they were created with.
#[napi(catch_unwind)]
pub fn clear_fonts() {
  oicana_ffi_core::clear_fonts();
}

/// Limits applied when reading a packed template zip. Missing values keep the defaults.
#[napi(object)]
pub struct ZipLimits {
  /// Maximum number of zip entries.
  pub max_entries: Option<i64>,
  /// Maximum total decompressed size in bytes.
  pub max_total_decompressed_bytes: Option<i64>,
}

fn into_core_limits(limits: Option<ZipLimits>) -> Result<Option<oicana_ffi_core::ZipLimits>> {
  let Some(limits) = limits else {
    return Ok(None);
  };
  oicana_ffi_core::ZipLimits::from_signed(limits.max_entries, limits.max_total_decompressed_bytes)
    .map_err(|error| Error::from_reason(error.to_string()))
}

impl From<CompilationMode> for oicana_ffi_core::CompilationMode {
  fn from(value: CompilationMode) -> Self {
    match value {
      CompilationMode::Production => oicana_ffi_core::CompilationMode::Production,
      CompilationMode::Development => oicana_ffi_core::CompilationMode::Development,
    }
  }
}

/// A blob with its metadata.
#[napi(object)]
pub struct BlobWithMetadata {
  /// The byte content of the blob.
  pub bytes: Uint8Array,
  /// Metadata of the blob.
  pub meta: String,
}

fn into_core_blobs(
  blobs: HashMap<String, BlobWithMetadata>,
) -> HashMap<String, oicana_ffi_core::BlobWithMetadata> {
  blobs
    .into_iter()
    .map(|(key, value)| {
      (
        key,
        oicana_ffi_core::BlobWithMetadata {
          bytes: value.bytes.to_vec(),
          meta: value.meta,
        },
      )
    })
    .collect()
}

fn into_napi_err(error: oicana_ffi_core::FfiError) -> Error {
  Error::from_reason(error.to_string())
}

/// Stop panics from unwinding across the napi boundary on libuv worker threads
/// and report them as JS errors instead.
fn catch_panic<T>(body: impl FnOnce() -> Result<T>) -> Result<T> {
  std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).unwrap_or_else(|payload| {
    Err(Error::from_reason(
      oicana_ffi_core::panic_message(&*payload).to_string(),
    ))
  })
}
