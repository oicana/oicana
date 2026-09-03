//! This crate defines FFI bindings for PDF templating from C#

use std::collections::HashMap;
use std::slice;

use interoptopus::patterns::slice::FFISlice;
use interoptopus::patterns::string::AsciiPointer;
use interoptopus::{ffi_function, ffi_type, function, Inventory, InventoryBuilder};
use oicana_ffi_core::{panic_message, swallow_panic};

/// Configure automatic cache eviction after each compilation.
///
/// # Parameters
///
/// `max_age` (start value: 10) - Maximum age threshold, or null to disable:
///   - `null` - Disables cache eviction (cache never cleared)
///   - `0` - Clears all cache entries with every eviction
///   - `1` - Keeps only entries used since the last eviction
///   - `n` - Keeps entries used within the last n evictions
#[ffi_function]
#[no_mangle]
pub extern "C" fn configure_automatic_cache_eviction(max_age: i64) {
    swallow_panic(|| {
        let max_age = if max_age < 0 {
            None
        } else {
            Some(max_age as usize)
        };
        oicana_ffi_core::configure_automatic_cache_eviction(max_age);
    });
}

/// Manually evict the comemo cache with the given age threshold.
///
/// This directly calls the underlying eviction with the specified age,
/// regardless of the configured default age.
///
/// # Parameters
///
/// * `max_age` - Maximum age threshold for eviction
///
/// Calls with negative `max_age` are ignored.
#[ffi_function]
#[no_mangle]
pub extern "C" fn evict_cache(max_age: i64) {
    swallow_panic(|| {
        if max_age >= 0 {
            oicana_ffi_core::evict_cache(max_age as usize);
        }
    });
}

/// Register a single font from its raw file content.
///
/// Returns the number of font faces that were added, so `0` means the data held
/// no font Typst can read. Returns `-1` if the call panicked.
///
/// # Safety
///
/// The caller is responsible for ensuring that `font` points to valid, properly
/// aligned and initialized data that is not modified concurrently.
#[ffi_function]
#[no_mangle]
pub unsafe extern "C" fn unsafe_register_font(font: Buffer) -> i64 {
    catch_panic_with(-1, || {
        let data = unsafe { slice_from_buffer(font) };
        oicana_ffi_core::register_font(data.to_vec()) as i64
    })
}

/// Register a single font file by path, not retaining its data until it is used.
///
/// Returns the number of font faces that were added. Returns `-1` if the path is
/// not valid UTF-8 or the call panicked.
#[ffi_function]
#[no_mangle]
pub extern "C" fn register_font_path(path: AsciiPointer) -> i64 {
    catch_panic_with(-1, || {
        let Ok(path) = path.as_str() else {
            return -1;
        };
        oicana_ffi_core::register_font_paths(vec![path.into()]) as i64
    })
}

/// All font faces currently registered by the host, as a JSON array of
/// `{ "family": ..., "path": ... }` objects.
///
/// Check if the returned buffer is an error before interpreting the content.
#[ffi_function]
#[no_mangle]
pub extern "C" fn registered_fonts() -> Buffer {
    catch_panic(
        || match serde_json::to_string(&oicana_ffi_core::registered_fonts()) {
            Ok(json) => Buffer::from_ok_string(json),
            Err(error) => {
                Buffer::from_error(format!("Failed to serialize registered fonts: {error}"))
            }
        },
    )
}

/// Drop all fonts registered by the host.
///
/// Templates that are already registered keep the fonts they were created with.
#[ffi_function]
#[no_mangle]
pub extern "C" fn clear_fonts() {
    swallow_panic(oicana_ffi_core::clear_fonts);
}

/// Register a template for the given identifier
///
/// After a successful call to this method, use [`unsafe_compile_template()`] for compiling
/// with improved performance.
///
/// # Safety
///
/// The caller is responsible for ensuring that the provided
/// `template`, `input`, and `banner` pointers are valid and non-null, and that
/// the `input` and `banner` data is properly aligned and initialized.
///
/// Additionally, the caller must ensure that no inputs are modified
/// concurrently while this function is executing.
#[ffi_function]
#[no_mangle]
pub unsafe extern "C" fn unsafe_register_template(
    template: AsciiPointer,
    files: Buffer,
    json_inputs: FFISlice<FfiJsonInput>,
    blob_inputs: FFISlice<FfiBlobInput>,
    compilation_options: CompilationOptions,
    limits: FfiZipLimits,
) -> Buffer {
    catch_panic(|| {
        let template = match template.as_str() {
            Ok(template) => template,
            Err(error) => return Buffer::from_error(format!("Invalid template ID: {error:?}")),
        };
        let files = unsafe { slice_from_buffer(files) };
        let (json_map, blob_map) = match unsafe { parse_inputs(json_inputs, blob_inputs) } {
            Ok(parsed) => parsed,
            Err(error) => return error,
        };

        Buffer::from_string_result(oicana_ffi_core::register_template(
            template,
            files,
            json_map,
            blob_map,
            compilation_options.mode.into(),
            limits.into(),
        ))
    })
}

/// Compile and export the given template once.
///
/// This method does not do any caching. If you want faster compilations,
/// prepare your templates by registering them with [`unsafe_register_template`]
/// and then calling [`unsafe_compile_template`] with the same identifier.
///
/// # Safety
///
/// The caller is responsible for ensuring that the provided
/// `files`, `json_inputs`, and `blob_inputs` pointers are valid and non-null,
/// and that all data is properly aligned and initialized.
///
/// Additionally, the caller must ensure that no inputs are modified
/// concurrently while this function is executing.
#[ffi_function]
#[no_mangle]
pub unsafe extern "C" fn unsafe_export_template_once(
    files: Buffer,
    json_inputs: FFISlice<FfiJsonInput>,
    blob_inputs: FFISlice<FfiBlobInput>,
    compile_options: CompilationOptions,
    export_options: ExportOptions,
    page_range: FfiPageRange,
    limits: FfiZipLimits,
) -> ExportOnceBuffers {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let files = unsafe { slice_from_buffer(files) };
        let (json_map, blob_map) = match unsafe { parse_inputs(json_inputs, blob_inputs) } {
            Ok(parsed) => parsed,
            Err(error) => return ExportOnceBuffers::from_error_buffer(error),
        };

        match oicana_ffi_core::export_once(
            files,
            json_map,
            blob_map,
            compile_options.mode.into(),
            export_options.into(),
            page_range.into(),
            limits.into(),
        ) {
            Ok(result) => ExportOnceBuffers {
                document: Buffer::from_ok(result.bytes),
                warnings: match result.warnings {
                    Some(warnings) => Buffer::from_ok_string(warnings),
                    None => Buffer::from_ok(Vec::new()),
                },
            },
            Err(error) => ExportOnceBuffers::from_error(error.to_string()),
        }
    }))
    .unwrap_or_else(|payload| {
        ExportOnceBuffers::from_error(format!(
            "internal panic: {}",
            panic_message(payload.as_ref())
        ))
    })
}

/// Compile the template with the given identifier
///
/// This method requires a previous successful call to [`unsafe_register_template()`].
/// Check if the returned buffer is an error before interpreting the content.
///
/// # Safety
///
/// The caller is responsible for ensuring that the provided
/// `template`, `input`, and `banner` pointers are valid and non-null, and that
/// the `input` and `banner` data is properly aligned and initialized.
///
/// Additionally, the caller must ensure that the blob input buffers are not modified
/// concurrently while this function is executing.
#[ffi_function]
#[no_mangle]
pub unsafe extern "C" fn unsafe_compile_template(
    template: AsciiPointer,
    json_inputs: FFISlice<FfiJsonInput>,
    blob_inputs: FFISlice<FfiBlobInput>,
    compilation_options: CompilationOptions,
) -> Buffer {
    catch_panic(|| {
        let template = match template.as_str() {
            Ok(template) => template,
            Err(error) => return Buffer::from_error(format!("Invalid template ID: {error:?}")),
        };
        let (json_map, blob_map) = match unsafe { parse_inputs(json_inputs, blob_inputs) } {
            Ok(parsed) => parsed,
            Err(error) => return error,
        };

        Buffer::from_string_result(oicana_ffi_core::compile_template(
            template,
            json_map,
            blob_map,
            compilation_options.mode.into(),
        ))
    })
}

/// Export the given document
///
/// # Safety
///
/// The caller is responsible for ensuring that the provided
/// `document_id` pointer is valid and non-null
#[ffi_function]
#[no_mangle]
pub unsafe extern "C" fn unsafe_export_document(
    document_id: AsciiPointer,
    export_options: ExportOptions,
    page_range: FfiPageRange,
) -> Buffer {
    catch_panic(|| {
        let document_id = match document_id.as_str() {
            Ok(document_id) => document_id,
            Err(error) => return Buffer::from_error(format!("Invalid document ID: {error:?}")),
        };
        Buffer::from_bytes_result(oicana_ffi_core::export_document(
            document_id,
            export_options.into(),
            page_range.into(),
        ))
    })
}

/// Load the manifest of the given template.
///
/// This method requires a previous successful call to [`unsafe_register_template`].
/// Check if the returned buffer is an error before interpreting the content.
#[ffi_function]
#[no_mangle]
pub extern "C" fn manifest(template: AsciiPointer) -> Buffer {
    catch_panic(|| {
        let template = match template.as_str() {
            Ok(template) => template,
            Err(error) => return Buffer::from_error(format!("{error:?}")),
        };
        Buffer::from_string_result(oicana_ffi_core::manifest(template))
    })
}

/// Return the sizes (in points) of every page of a compiled document as a JSON
/// array of `{ "width": number, "height": number }`.
///
/// This method requires a previous successful call producing the `document_id`.
#[ffi_function]
#[no_mangle]
pub extern "C" fn document_pages(document_id: AsciiPointer) -> Buffer {
    catch_panic(|| {
        let document_id = match document_id.as_str() {
            Ok(document_id) => document_id,
            Err(error) => return Buffer::from_error(format!("{error:?}")),
        };
        Buffer::from_string_result(oicana_ffi_core::document_pages(document_id))
    })
}

/// Load the source at the given path in the template.
///
/// This method requires a previous successful call to [`unsafe_register_template`].
/// Check if the returned buffer is an error before interpreting the content.
#[ffi_function]
#[no_mangle]
pub extern "C" fn get_source(template: AsciiPointer, path: AsciiPointer) -> Buffer {
    catch_panic(|| {
        let template = match template.as_str() {
            Ok(template) => template,
            Err(error) => return Buffer::from_error(format!("Invalid template ID: {error:?}")),
        };
        let path = match path.as_str() {
            Ok(path) => path,
            Err(error) => return Buffer::from_error(format!("Invalid path: {error:?}")),
        };
        Buffer::from_string_result(oicana_ffi_core::get_source(template, path))
    })
}

/// Load the file at the given path in the template.
///
/// This method requires a previous successful call to [`unsafe_register_template`].
/// Check if the returned buffer is an error before interpreting the content.
#[ffi_function]
#[no_mangle]
pub extern "C" fn get_file(template: AsciiPointer, path: AsciiPointer) -> Buffer {
    catch_panic(|| {
        let template = match template.as_str() {
            Ok(template) => template,
            Err(error) => return Buffer::from_error(format!("Invalid template ID: {error:?}")),
        };
        let path = match path.as_str() {
            Ok(path) => path,
            Err(error) => return Buffer::from_error(format!("Invalid path: {error:?}")),
        };
        Buffer::from_bytes_result(oicana_ffi_core::get_file(template, path))
    })
}

/// Frees a buffer allocated by `compile_template`.
///
/// # Safety
///
/// This function is unsafe because it assumes the following:
///
/// 1. If [`Buffer::data`] is non-null, it must point to memory allocated by
///    Rust which was not previously freed. Null buffers are ignored.
///
/// 2. No other pointers to the memory should be used after this function has been called.
///
/// 3. This function must be called from a context where it is safe to free memory, ensuring
///    no concurrent accesses.
#[ffi_function]
#[no_mangle]
pub unsafe extern "C" fn unsafe_free_buffer(buffer: Buffer) {
    if buffer.data.is_null() {
        return;
    }
    unsafe {
        let _boxed_data = Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            buffer.data,
            buffer.len as usize,
        ));
    }
}

/// Enable or disable JSON schema validation for the given template.
///
/// When enabled (the default), JSON inputs are validated against their schemas
/// before compilation.
#[ffi_function]
#[no_mangle]
pub extern "C" fn set_validate_inputs(template: AsciiPointer, validate: bool) -> Buffer {
    catch_panic(|| {
        let template = match template.as_str() {
            Ok(template) => template,
            Err(error) => return Buffer::from_error(format!("{error:?}")),
        };
        Buffer::from_unit_result(oicana_ffi_core::set_validate_inputs(template, validate))
    })
}

/// Configure Oicana.
#[ffi_function]
#[no_mangle]
pub extern "C" fn configure(config: Config) -> Buffer {
    catch_panic(|| {
        oicana_ffi_core::configure_diagnostic_color(config.color.into());
        Buffer::from_ok(Vec::new())
    })
}

/// Remove the document from the cache.
#[ffi_function]
#[no_mangle]
pub extern "C" fn remove_document(document_id: AsciiPointer) -> Buffer {
    catch_panic(|| {
        let document_id = match document_id.as_str() {
            Ok(document_id) => document_id,
            Err(error) => return Buffer::from_error(format!("{error:?}")),
        };
        oicana_ffi_core::remove_document(document_id);
        Buffer::from_ok(Vec::new())
    })
}

/// Return any compilation warnings produced for the given document.
///
/// On success the buffer contains either the warnings text (UTF-8) or is
/// empty when there were no warnings. Warnings are cleared together with
/// the document by [`remove_document`].
#[ffi_function]
#[no_mangle]
pub extern "C" fn get_warnings(document_id: AsciiPointer) -> Buffer {
    catch_panic(|| {
        let document_id = match document_id.as_str() {
            Ok(document_id) => document_id,
            Err(error) => return Buffer::from_error(format!("{error:?}")),
        };
        match oicana_ffi_core::get_warnings(document_id) {
            Some(warnings) => Buffer::from_ok_string(warnings),
            None => Buffer::from_ok(Vec::new()),
        }
    })
}

/// Clear the specified template from the internal cache.
///
/// This method requires a previous successful call to [`unsafe_register_template`].
/// Check if the returned buffer is an error before interpreting the content.
#[ffi_function]
#[no_mangle]
pub extern "C" fn remove_world(template_id: AsciiPointer) -> Buffer {
    catch_panic(|| {
        let template_id = match template_id.as_str() {
            Ok(template_id) => template_id,
            Err(error) => return Buffer::from_error(format!("{error:?}")),
        };
        oicana_ffi_core::remove_world(template_id);
        Buffer::from_ok(Vec::new())
    })
}

/// Access to a piece of Rust memory.
///
/// If [`Self::error`] is `true`, [`Self::data`] will point to a UTF-8 encoded error message.
#[ffi_type]
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Buffer {
    /// Pointer to the beginning of the buffer data.
    pub data: *mut u8,
    /// Whether this buffer is an error.
    pub error: bool,
    /// Length of the buffer data.
    pub len: u32,
}

impl Buffer {
    fn from_error(error_string: String) -> Self {
        Buffer {
            error: true,
            ..Buffer::from_ok_string(error_string)
        }
    }

    fn from_ok_string(string: String) -> Self {
        Buffer::from_ok(string.into_bytes())
    }

    fn from_ok(value: Vec<u8>) -> Self {
        let Ok(len) = u32::try_from(value.len()) else {
            return Buffer::from_error(format!(
                "output of {} bytes exceeds the {} byte limit of the FFI buffer",
                value.len(),
                u32::MAX
            ));
        };
        let mut buf = value.into_boxed_slice();
        let data = buf.as_mut_ptr();
        std::mem::forget(buf);

        Buffer {
            data,
            len,
            error: false,
        }
    }

    fn from_bytes_result(result: Result<Vec<u8>, oicana_ffi_core::FfiError>) -> Self {
        match result {
            Ok(bytes) => Buffer::from_ok(bytes),
            Err(error) => Buffer::from_error(error.to_string()),
        }
    }

    fn from_string_result(result: Result<String, oicana_ffi_core::FfiError>) -> Self {
        match result {
            Ok(string) => Buffer::from_ok_string(string),
            Err(error) => Buffer::from_error(error.to_string()),
        }
    }

    fn from_unit_result(result: Result<(), oicana_ffi_core::FfiError>) -> Self {
        match result {
            Ok(()) => Buffer::from_ok(Vec::new()),
            Err(error) => Buffer::from_error(error.to_string()),
        }
    }
}

/// Buffers returned by [`unsafe_export_template_once`].
#[ffi_type]
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ExportOnceBuffers {
    /// The exported document, or an error.
    pub document: Buffer,
    /// Compilation warnings; empty when there were none.
    pub warnings: Buffer,
}

impl ExportOnceBuffers {
    fn from_error(error_string: String) -> Self {
        Self::from_error_buffer(Buffer::from_error(error_string))
    }

    fn from_error_buffer(error: Buffer) -> Self {
        ExportOnceBuffers {
            document: error,
            warnings: Buffer::from_ok(Vec::new()),
        }
    }
}

/// Limits for packed templates.
#[ffi_type]
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct FfiZipLimits {
    /// Maximum number of zip entries. `-1` uses the default.
    pub max_entries: i64,
    /// Maximum total decompressed size in bytes. `-1` uses the default.
    pub max_total_decompressed_bytes: i64,
}

impl From<FfiZipLimits> for Option<oicana_ffi_core::ZipLimits> {
    fn from(limits: FfiZipLimits) -> Self {
        oicana_ffi_core::ZipLimits::from_sentinels(
            limits.max_entries,
            limits.max_total_decompressed_bytes,
        )
    }
}

/// A collection of string pairs representing JSON inputs
#[ffi_type]
#[repr(C)]
#[derive(Debug)]
pub struct FfiJsonInput<'a> {
    /// String containing the json payload of this input.
    pub data: AsciiPointer<'a>,
    /// Identifier of the input definition this input value belongs to.
    pub key: AsciiPointer<'a>,
}

/// A collection of string keys with Buffers representing blob inputs
#[ffi_type]
#[repr(C)]
#[derive(Debug)]
pub struct FfiBlobInput<'a> {
    /// Buffer containing the main data of the blob input.
    pub data: Buffer,
    /// Identifier of the input definition this input value belongs to.
    pub key: AsciiPointer<'a>,
    /// Metadata of the blob input as json.
    pub meta: AsciiPointer<'a>,
}

/// Formats that an Oicana template can be exported into.
#[ffi_type]
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum CompilationTarget {
    /// Export to a PDF file.
    ///
    /// The exported standard can be configured in the template manifest
    /// via [tool.oicana.export.pdf] section. Defaults to PDF/A-3b.
    Pdf,
    /// Export to a png image.
    ///
    /// The image is not optimized for file size to speed up compilation.
    Png,
    /// Export to an SVG file.
    Svg,
}

/// The mode of compilation
#[ffi_type]
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CompilationMode {
    /// Use development values for inputs if an input is not explicitly set.
    /// If there is no development value, fall back to the default value.
    Development,
    /// If an input is not set, use the default value if available.
    Production,
}

impl From<CompilationMode> for oicana_ffi_core::CompilationMode {
    fn from(mode: CompilationMode) -> Self {
        match mode {
            CompilationMode::Development => oicana_ffi_core::CompilationMode::Development,
            CompilationMode::Production => oicana_ffi_core::CompilationMode::Production,
        }
    }
}

/// Options for compiling the template
#[ffi_type]
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CompilationOptions {
    /// The mode of compilation
    pub mode: CompilationMode,
}

/// Options for exporting the template
#[ffi_type]
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ExportOptions {
    /// Formats that an Oicana template can be compiled into.
    pub target: CompilationTarget,
    /// Pixels per pt
    /// Only used for PNG export
    pub px_per_pt: f32,
}

impl From<ExportOptions> for oicana_ffi_core::ExportFormat {
    fn from(opts: ExportOptions) -> Self {
        match opts.target {
            CompilationTarget::Pdf => oicana_ffi_core::ExportFormat::Pdf,
            CompilationTarget::Png => oicana_ffi_core::ExportFormat::Png {
                pixels_per_pt: opts.px_per_pt,
            },
            CompilationTarget::Svg => oicana_ffi_core::ExportFormat::Svg,
        }
    }
}

/// A contiguous, 0-based inclusive range of pages to export.
///
/// Each bound uses `-1` to mean "open" (the document's first/last page). The
/// sentinel `{ start: -1, end: -1 }` therefore selects the whole document.
#[ffi_type]
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct FfiPageRange {
    /// First page index to export (0-based, inclusive). `-1` selects from the first page.
    pub start: i64,
    /// Last page index to export (0-based, inclusive). `-1` selects up to the last page.
    pub end: i64,
}

impl From<FfiPageRange> for Option<oicana_ffi_core::PageRange> {
    fn from(range: FfiPageRange) -> Self {
        if range.start < 0 && range.end < 0 {
            return None;
        }
        Some(oicana_ffi_core::PageRange {
            start: (range.start >= 0).then_some(range.start as usize),
            end: (range.end >= 0).then_some(range.end as usize),
        })
    }
}

/// Color mode for compilation diagnostics.
#[ffi_type]
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum DiagnosticColor {
    /// No colors in diagnostic output
    None,
    /// ANSI codes for colors in diagnostic output
    Ansi,
}

impl From<DiagnosticColor> for oicana_ffi_core::DiagnosticColor {
    fn from(value: DiagnosticColor) -> Self {
        match value {
            DiagnosticColor::Ansi => oicana_ffi_core::DiagnosticColor::Ansi,
            DiagnosticColor::None => oicana_ffi_core::DiagnosticColor::None,
        }
    }
}

/// Oicana Configuration.
#[ffi_type]
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Config {
    /// Coloring for diagnostics like warnings and errors
    pub color: DiagnosticColor,
}

unsafe fn slice_from_buffer<'a>(buffer: Buffer) -> &'a [u8] {
    if buffer.data.is_null() || buffer.len == 0 {
        return &[];
    }
    unsafe { slice::from_raw_parts::<u8>(buffer.data, buffer.len as usize) }
}

type ParsedInputs = (
    HashMap<String, String>,
    HashMap<String, oicana_ffi_core::BlobWithMetadata>,
);

unsafe fn parse_inputs(
    json_inputs: FFISlice<FfiJsonInput>,
    blob_inputs: FFISlice<FfiBlobInput>,
) -> Result<ParsedInputs, Buffer> {
    let mut json_map = HashMap::new();
    for input in json_inputs.iter() {
        let key = input
            .key
            .as_str()
            .map_err(|error| Buffer::from_error(format!("Invalid JSON input key: {error:?}")))?;
        let data = input
            .data
            .as_str()
            .map_err(|error| Buffer::from_error(format!("Invalid JSON input data: {error:?}")))?;
        json_map.insert(key.to_owned(), data.to_owned());
    }

    let mut blob_map = HashMap::new();
    for input in blob_inputs.iter() {
        let key = input
            .key
            .as_str()
            .map_err(|error| Buffer::from_error(format!("Invalid blob input key: {error:?}")))?;
        let meta = input
            .meta
            .as_str()
            .map_err(|error| Buffer::from_error(format!("Invalid blob input meta: {error:?}")))?;
        let bytes = unsafe { slice_from_buffer(input.data) }.to_vec();
        blob_map.insert(
            key.to_owned(),
            oicana_ffi_core::BlobWithMetadata {
                bytes,
                meta: meta.to_owned(),
            },
        );
    }

    Ok((json_map, blob_map))
}

/// List methods for auto generated bindings
pub fn my_inventory() -> Inventory {
    InventoryBuilder::new()
        .register(function!(unsafe_compile_template))
        .register(function!(unsafe_export_template_once))
        .register(function!(unsafe_register_template))
        .register(function!(unsafe_export_document))
        .register(function!(manifest))
        .register(function!(document_pages))
        .register(function!(get_source))
        .register(function!(get_file))
        .register(function!(unsafe_free_buffer))
        .register(function!(remove_world))
        .register(function!(remove_document))
        .register(function!(get_warnings))
        .register(function!(configure))
        .register(function!(set_validate_inputs))
        .register(function!(configure_automatic_cache_eviction))
        .register(function!(evict_cache))
        .register(function!(unsafe_register_font))
        .register(function!(register_font_path))
        .register(function!(registered_fonts))
        .register(function!(clear_fonts))
        .inventory()
}

/// Run `body`, converting any panic into an error [`Buffer`].
///
/// Every `extern "C"` function must go through this (or [`swallow_panic`]):
/// unwinding across the C ABI boundary aborts the host .NET process.
fn catch_panic(body: impl FnOnce() -> Buffer) -> Buffer {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).unwrap_or_else(|payload| {
        Buffer::from_error(format!(
            "internal panic: {}",
            panic_message(payload.as_ref())
        ))
    })
}

/// Run `body`, returning `fallback` if it panics.
///
/// For entry points that return a plain value rather than a [`Buffer`], and so
/// have no channel to report the error through.
fn catch_panic_with<T>(fallback: T, body: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_becomes_error_buffer() {
        let buffer = catch_panic(|| panic!("something went wrong: {}", 42));
        assert!(buffer.error);
        let message = unsafe { std::str::from_utf8(slice_from_buffer(buffer)) }
            .unwrap()
            .to_owned();
        unsafe { unsafe_free_buffer(buffer) };
        assert_eq!(message, "internal panic: something went wrong: 42");
    }

    #[test]
    fn free_buffer_ignores_null_data() {
        unsafe {
            unsafe_free_buffer(Buffer {
                data: std::ptr::null_mut(),
                error: false,
                len: 0,
            })
        };
    }

    #[test]
    fn slice_from_null_buffer_is_empty() {
        let slice = unsafe {
            slice_from_buffer(Buffer {
                data: std::ptr::null_mut(),
                error: false,
                len: 42,
            })
        };
        assert!(slice.is_empty());
    }

    #[test]
    fn slice_from_zero_length_buffer_ignores_the_pointer() {
        let slice = unsafe {
            slice_from_buffer(Buffer {
                data: 0xdead_beef_usize as *mut u8,
                error: false,
                len: 0,
            })
        };
        assert!(slice.is_empty());
    }

    #[cfg(all(unix, target_pointer_width = "64"))]
    #[test]
    fn oversized_output_becomes_error_buffer() {
        let buffer = Buffer::from_ok(vec![0_u8; u32::MAX as usize + 1]);
        assert!(buffer.error);
        let message = unsafe { std::str::from_utf8(slice_from_buffer(buffer)) }
            .unwrap()
            .to_owned();
        unsafe { unsafe_free_buffer(buffer) };
        assert!(message.contains("exceeds"), "unexpected message: {message}");
    }
}
