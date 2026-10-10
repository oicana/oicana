//! UniFFI bindings for Oicana.
//!
//! Exposes the API of `oicana_ffi_core` through UniFFI proc-macros.

use std::collections::HashMap;

mod error;
mod template;
mod types;

pub use error::OicanaError;
pub use template::{CompiledDocument, Template};
pub use types::{
    BlobInput, CompilationMode, DiagnosticColor, ExportFormat, ExportOnceResult, PageRange,
    PageSize, RegisteredFont, ZipLimits,
};

uniffi::setup_scaffolding!();

/// Compile a packed template once and export it, without caching anything.
///
/// For repeated compilations of the same template, use [`Template`].
#[uniffi::export(default(pages = None, limits = None))]
pub fn export_once(
    files: Vec<u8>,
    json_inputs: HashMap<String, String>,
    blob_inputs: HashMap<String, BlobInput>,
    mode: CompilationMode,
    format: ExportFormat,
    pages: Option<PageRange>,
    limits: Option<ZipLimits>,
) -> Result<ExportOnceResult, OicanaError> {
    let result = oicana_ffi_core::export_once(
        &files,
        json_inputs,
        types::core_blobs(blob_inputs),
        mode.into(),
        format.into(),
        pages.map(Into::into),
        limits.and_then(Into::into),
    )?;
    Ok(ExportOnceResult {
        bytes: result.bytes,
        warnings: result.warnings,
    })
}

/// Register fonts from their raw file content. Returns the number of faces added.
///
/// Templates keep the fonts that were registered when they were created, so register
/// fonts first. Nothing deduplicates.
#[uniffi::export]
pub fn register_fonts(fonts: Vec<Vec<u8>>) -> u64 {
    oicana_ffi_core::register_fonts(fonts) as u64
}

/// Register fonts from files, or from every font file in a directory tree. Returns the
/// number of faces added.
#[uniffi::export]
pub fn register_font_paths(paths: Vec<String>) -> u64 {
    oicana_ffi_core::register_font_paths(paths.into_iter().map(Into::into).collect()) as u64
}

/// All font faces registered by the host.
#[uniffi::export]
pub fn registered_fonts() -> Vec<RegisteredFont> {
    oicana_ffi_core::registered_fonts()
        .into_iter()
        .map(Into::into)
        .collect()
}

/// Drop all fonts registered by the host. Existing templates keep theirs.
#[uniffi::export]
pub fn clear_fonts() {
    oicana_ffi_core::clear_fonts();
}

/// Set the coloring of compilation diagnostics for all templates.
#[uniffi::export]
pub fn configure_diagnostic_color(color: DiagnosticColor) {
    oicana_ffi_core::configure_diagnostic_color(color.into());
}

/// Configure cache eviction after each compilation.
///
/// Keeps cache entries used within the last `max_age` evictions; `None` disables
/// automatic eviction. Defaults to 10.
#[uniffi::export]
pub fn configure_automatic_cache_eviction(max_age: Option<u64>) {
    oicana_ffi_core::configure_automatic_cache_eviction(max_age.map(types::saturating_usize));
}

/// Evict cache entries not used within the last `max_age` evictions.
#[uniffi::export]
pub fn evict_cache(max_age: u64) {
    oicana_ffi_core::evict_cache(types::saturating_usize(max_age));
}

/// Version of the Oicana library.
#[uniffi::export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
