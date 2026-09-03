//! The Python integration of Oicana.
//!
//! You will want to use this through the Python package `oicana`.

use std::collections::HashMap;
use std::path::PathBuf;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

/// Compilation mode enum
#[pyclass(eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompilationMode {
    Production,
    Development,
}

impl From<CompilationMode> for oicana_ffi_core::CompilationMode {
    fn from(mode: CompilationMode) -> Self {
        match mode {
            CompilationMode::Production => oicana_ffi_core::CompilationMode::Production,
            CompilationMode::Development => oicana_ffi_core::CompilationMode::Development,
        }
    }
}

/// Blob input with metadata
#[pyclass]
pub struct BlobWithMetadata {
    #[pyo3(get)]
    pub bytes: Py<PyBytes>,
    #[pyo3(get)]
    pub meta: String, // JSON string
}

#[pymethods]
impl BlobWithMetadata {
    #[new]
    fn new(bytes: Py<PyBytes>, meta: String) -> Self {
        Self { bytes, meta }
    }
}

/// Configure automatic cache eviction after each compilation.
///
/// # Parameters
///
/// `max_age` (start value: 10) - Maximum age threshold, or null to disable:
///   - `null` - Disables cache eviction (cache never cleared)
///   - `0` - Clears all cache entries with every eviction
///   - `1` - Keeps only entries used since the last eviction
///   - `n` - Keeps entries used within the last n evictions
#[pyfunction]
#[pyo3(signature = (max_age=None))]
fn configure_automatic_cache_eviction(max_age: Option<usize>) {
    oicana_ffi_core::configure_automatic_cache_eviction(max_age);
}

/// Manually evict the comemo cache with the given age threshold.
///
/// This directly calls the underlying eviction with the specified age,
/// regardless of the configured default age.
#[pyfunction]
fn evict_cache(py: Python<'_>, max_age: usize) {
    py.detach(|| oicana_ffi_core::evict_cache(max_age));
}

/// Register the given template. This will read the template files as a PackedTemplate and
/// compile it once with the given inputs. The Typst World will be cached and reused for
/// subsequent calls to the other methods with the same template identifier.
#[pyfunction]
#[pyo3(signature = (template, files, json_inputs, blob_inputs, compilation_mode, max_entries=None, max_total_decompressed_bytes=None))]
#[allow(clippy::too_many_arguments)]
fn register_template(
    py: Python<'_>,
    template: String,
    files: &Bound<'_, PyBytes>,
    json_inputs: HashMap<String, String>,
    blob_inputs: &Bound<'_, PyDict>,
    compilation_mode: CompilationMode,
    max_entries: Option<u64>,
    max_total_decompressed_bytes: Option<u64>,
) -> PyResult<String> {
    let blobs = into_core_blobs(py, blob_inputs)?;
    let files = files.as_bytes();
    py.detach(|| {
        oicana_ffi_core::register_template(
            &template,
            files,
            json_inputs,
            blobs,
            compilation_mode.into(),
            oicana_ffi_core::ZipLimits::from_optional(max_entries, max_total_decompressed_bytes),
        )
    })
    .map_err(into_py_err)
}

/// Compile and export the given template once, without caching anything.
///
/// Returns a `(document_bytes, warnings)` tuple where `warnings` is `None` if
/// there were none.
#[pyfunction]
#[pyo3(signature = (files, json_inputs, blob_inputs, compilation_mode, export_format, page_range=None, max_entries=None, max_total_decompressed_bytes=None))]
#[allow(clippy::too_many_arguments)]
fn export_once<'py>(
    py: Python<'py>,
    files: &Bound<'_, PyBytes>,
    json_inputs: HashMap<String, String>,
    blob_inputs: &Bound<'_, PyDict>,
    compilation_mode: CompilationMode,
    export_format: String,
    page_range: Option<String>,
    max_entries: Option<u64>,
    max_total_decompressed_bytes: Option<u64>,
) -> PyResult<(Bound<'py, PyBytes>, Option<String>)> {
    let format = oicana_ffi_core::parse_export_format(&export_format).map_err(into_py_err)?;
    let page = oicana_ffi_core::parse_page_range(page_range.as_deref()).map_err(into_py_err)?;
    let blobs = into_core_blobs(py, blob_inputs)?;
    let files = files.as_bytes();
    let result = py
        .detach(|| {
            oicana_ffi_core::export_once(
                files,
                json_inputs,
                blobs,
                compilation_mode.into(),
                format,
                page,
                oicana_ffi_core::ZipLimits::from_optional(
                    max_entries,
                    max_total_decompressed_bytes,
                ),
            )
        })
        .map_err(into_py_err)?;
    Ok((PyBytes::new(py, &result.bytes), result.warnings))
}

/// Configure the coloring of compilation diagnostics like warnings and errors.
#[pyfunction]
fn configure_diagnostic_color(ansi: bool) {
    let color = if ansi {
        oicana_ffi_core::DiagnosticColor::Ansi
    } else {
        oicana_ffi_core::DiagnosticColor::None
    };
    oicana_ffi_core::configure_diagnostic_color(color);
}

/// Register fonts from their raw file content.
///
/// Returns the number of font faces that were added.
#[pyfunction]
fn register_fonts(py: Python<'_>, fonts: Vec<Vec<u8>>) -> usize {
    py.detach(|| oicana_ffi_core::register_fonts(fonts))
}

/// Register fonts from files on disk, reading their data lazily.
///
/// Paths that cannot be read or hold no font are skipped.
/// Returns the number of font faces that were added.
#[pyfunction]
fn register_font_paths(py: Python<'_>, paths: Vec<PathBuf>) -> usize {
    py.detach(|| oicana_ffi_core::register_font_paths(paths))
}

/// All font faces currently registered, as `(family, path)` tuples.
///
/// `path` is `None` for fonts registered from memory.
#[pyfunction]
fn registered_fonts() -> Vec<(String, Option<String>)> {
    oicana_ffi_core::registered_fonts()
        .into_iter()
        .map(|font| (font.family, font.path))
        .collect()
}

/// Drop all registered fonts.
///
/// Templates that are already registered keep the fonts they were created with.
#[pyfunction]
fn clear_fonts() {
    oicana_ffi_core::clear_fonts();
}

/// Compile the identified template with the given inputs.
///
/// Calling this method requires a previous call to `register_template` with the same template
/// identifier.
#[pyfunction]
fn compile_template(
    py: Python<'_>,
    template: String,
    json_inputs: HashMap<String, String>,
    blob_inputs: &Bound<'_, PyDict>,
    compilation_mode: CompilationMode,
) -> PyResult<String> {
    let blobs = into_core_blobs(py, blob_inputs)?;
    py.detach(|| {
        oicana_ffi_core::compile_template(&template, json_inputs, blobs, compilation_mode.into())
    })
    .map_err(into_py_err)
}

/// Load the manifest of the given template.
///
/// Calling this method requires a previous call to `register_template` with the same template
/// identifier.
#[pyfunction]
fn manifest(py: Python<'_>, template: String) -> PyResult<String> {
    py.detach(|| oicana_ffi_core::manifest(&template))
        .map_err(into_py_err)
}

/// Return the sizes (in points) of every page of a compiled document as a JSON
/// array of `{ "width": float, "height": float }`.
#[pyfunction]
fn document_pages(py: Python<'_>, document_id: String) -> PyResult<String> {
    py.detach(|| oicana_ffi_core::document_pages(&document_id))
        .map_err(into_py_err)
}

/// Load the source of the given file in the template.
///
/// Calling this method requires a previous call to `register_template` with the same template
/// identifier.
#[pyfunction]
fn get_source(py: Python<'_>, template: String, file: String) -> PyResult<String> {
    py.detach(|| oicana_ffi_core::get_source(&template, &file))
        .map_err(into_py_err)
}

/// Load the binary file content from the template.
///
/// Calling this method requires a previous call to `register_template` with the same template
/// identifier.
#[pyfunction]
fn get_file(py: Python<'_>, template: String, file: String) -> PyResult<Bound<'_, PyBytes>> {
    let bytes = py
        .detach(|| oicana_ffi_core::get_file(&template, &file))
        .map_err(into_py_err)?;
    Ok(PyBytes::new(py, &bytes))
}

/// Export the given document
///
/// `page_range` is a JSON object `{ "start"?: int, "end"?: int }` with 0-based,
/// inclusive bounds. If not set, the whole document is exported.
///
/// Make sure to call `remove_document` with the document_id afterwards, to free the memory.
#[pyfunction]
#[pyo3(signature = (document_id, export_format, page_range=None))]
fn export_document(
    py: Python<'_>,
    document_id: String,
    export_format: String,
    page_range: Option<String>,
) -> PyResult<Bound<'_, PyBytes>> {
    let format = oicana_ffi_core::parse_export_format(&export_format).map_err(into_py_err)?;
    let page = oicana_ffi_core::parse_page_range(page_range.as_deref()).map_err(into_py_err)?;
    let bytes = py
        .detach(|| oicana_ffi_core::export_document(&document_id, format, page))
        .map_err(into_py_err)?;
    Ok(PyBytes::new(py, &bytes))
}

/// Remove the document from the cache.
#[pyfunction]
fn remove_document(py: Python<'_>, document_id: String) -> PyResult<()> {
    py.detach(|| oicana_ffi_core::remove_document(&document_id));
    Ok(())
}

/// Return any compilation warnings produced for the given document, or `None`
/// if there were none. Warnings are cleared when the document is removed.
#[pyfunction]
fn get_warnings(py: Python<'_>, document_id: String) -> Option<String> {
    py.detach(|| oicana_ffi_core::get_warnings(&document_id))
}

/// Enable or disable JSON schema validation for the given template.
///
/// When enabled (the default), JSON inputs are validated against their schemas
/// before compilation.
///
/// Calling this method requires a previous call to `register_template` with the same template
/// identifier.
#[pyfunction]
fn set_validate_inputs(py: Python<'_>, template: String, validate: bool) -> PyResult<()> {
    py.detach(|| oicana_ffi_core::set_validate_inputs(&template, validate))
        .map_err(into_py_err)
}

/// Remove the world from the cache.
///
/// The template will have to be registered again before it can be compiled again.
#[pyfunction]
fn remove_world(py: Python<'_>, template_id: String) -> PyResult<()> {
    py.detach(|| oicana_ffi_core::remove_world(&template_id));
    Ok(())
}

fn into_core_blobs(
    py: Python<'_>,
    blob_inputs: &Bound<'_, PyDict>,
) -> PyResult<HashMap<String, oicana_ffi_core::BlobWithMetadata>> {
    let mut blobs = HashMap::with_capacity(blob_inputs.len());
    for (key, value) in blob_inputs.iter() {
        let key: String = key.extract()?;
        let blob: Py<BlobWithMetadata> = value.extract()?;
        let blob = blob.bind(py).borrow();
        let bytes = blob.bytes.bind(py).as_bytes().to_vec();
        blobs.insert(
            key,
            oicana_ffi_core::BlobWithMetadata {
                bytes,
                meta: blob.meta.clone(),
            },
        );
    }
    Ok(blobs)
}

fn into_py_err(error: oicana_ffi_core::FfiError) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

#[pymodule]
fn oicana_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(register_template, m)?)?;
    m.add_function(wrap_pyfunction!(export_once, m)?)?;
    m.add_function(wrap_pyfunction!(configure_diagnostic_color, m)?)?;
    m.add_function(wrap_pyfunction!(compile_template, m)?)?;
    m.add_function(wrap_pyfunction!(export_document, m)?)?;
    m.add_function(wrap_pyfunction!(manifest, m)?)?;
    m.add_function(wrap_pyfunction!(document_pages, m)?)?;
    m.add_function(wrap_pyfunction!(get_source, m)?)?;
    m.add_function(wrap_pyfunction!(get_file, m)?)?;
    m.add_function(wrap_pyfunction!(remove_document, m)?)?;
    m.add_function(wrap_pyfunction!(get_warnings, m)?)?;
    m.add_function(wrap_pyfunction!(remove_world, m)?)?;
    m.add_function(wrap_pyfunction!(set_validate_inputs, m)?)?;
    m.add_function(wrap_pyfunction!(configure_automatic_cache_eviction, m)?)?;
    m.add_function(wrap_pyfunction!(evict_cache, m)?)?;
    m.add_function(wrap_pyfunction!(register_fonts, m)?)?;
    m.add_function(wrap_pyfunction!(register_font_paths, m)?)?;
    m.add_function(wrap_pyfunction!(registered_fonts, m)?)?;
    m.add_function(wrap_pyfunction!(clear_fonts, m)?)?;
    m.add_class::<CompilationMode>()?;
    m.add_class::<BlobWithMetadata>()?;
    Ok(())
}
