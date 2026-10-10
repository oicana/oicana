use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use oicana_ffi_core::{DocumentHandle, TemplateHandle};

use crate::error::OicanaError;
use crate::types::{
    core_blobs, BlobInput, CompilationMode, ExportFormat, PageRange, PageSize, ZipLimits,
};

static NEXT_TEMPLATE_ID: AtomicU64 = AtomicU64::new(0);

/// A registered template, ready for repeated compilation.
///
/// Dropping it removes the template from the cache. Documents compiled from it stay
/// usable.
#[derive(uniffi::Object)]
pub struct Template {
    id: String,
    handle: TemplateHandle,
    warnings: Option<String>,
}

#[uniffi::export]
impl Template {
    /// Register a packed template and compile it once with the given inputs as a warm-up.
    #[uniffi::constructor(default(limits = None))]
    pub fn new(
        files: Vec<u8>,
        json_inputs: HashMap<String, String>,
        blob_inputs: HashMap<String, BlobInput>,
        mode: CompilationMode,
        limits: Option<ZipLimits>,
    ) -> Result<Arc<Self>, OicanaError> {
        let id = format!(
            "uniffi-{}",
            NEXT_TEMPLATE_ID.fetch_add(1, Ordering::Relaxed)
        );
        let warm_up = oicana_ffi_core::register_template(
            &id,
            &files,
            json_inputs,
            core_blobs(blob_inputs),
            mode.into(),
            limits.and_then(Into::into),
        )?;
        let warnings = oicana_ffi_core::get_warnings(&warm_up);
        oicana_ffi_core::remove_document(&warm_up);
        let handle = oicana_ffi_core::lookup_template(&id)?;
        Ok(Arc::new(Template {
            id,
            handle,
            warnings,
        }))
    }

    /// Warnings of the warm-up compilation, if any.
    pub fn warnings(&self) -> Option<String> {
        self.warnings.clone()
    }

    /// Compile the template with the given inputs.
    pub fn compile(
        &self,
        json_inputs: HashMap<String, String>,
        blob_inputs: HashMap<String, BlobInput>,
        mode: CompilationMode,
    ) -> Result<Arc<CompiledDocument>, OicanaError> {
        let id = self
            .handle
            .compile(json_inputs, core_blobs(blob_inputs), mode.into())?;
        CompiledDocument::from_id(id).map(Arc::new)
    }

    /// The template manifest as JSON.
    pub fn manifest(&self) -> Result<String, OicanaError> {
        Ok(oicana_ffi_core::manifest(&self.id)?)
    }

    /// Source text of a file inside the template.
    pub fn source(&self, path: String) -> Result<String, OicanaError> {
        Ok(oicana_ffi_core::get_source(&self.id, &path)?)
    }

    /// Raw bytes of a file inside the template.
    pub fn file(&self, path: String) -> Result<Vec<u8>, OicanaError> {
        Ok(oicana_ffi_core::get_file(&self.id, &path)?)
    }

    /// Enable or disable validating JSON inputs against their schemas. Enabled by default.
    pub fn set_validate_inputs(&self, validate: bool) -> Result<(), OicanaError> {
        Ok(oicana_ffi_core::set_validate_inputs(&self.id, validate)?)
    }
}

impl Drop for Template {
    fn drop(&mut self) {
        oicana_ffi_core::remove_world(&self.id);
    }
}

/// A compiled document that can be exported repeatedly without compiling again.
///
/// Dropping it frees the document.
#[derive(uniffi::Object)]
pub struct CompiledDocument {
    id: String,
    handle: DocumentHandle,
    warnings: Option<String>,
}

impl CompiledDocument {
    fn from_id(id: String) -> Result<Self, OicanaError> {
        let handle = oicana_ffi_core::lookup_document(&id)?;
        let warnings = oicana_ffi_core::get_warnings(&id);
        Ok(CompiledDocument {
            id,
            handle,
            warnings,
        })
    }
}

#[uniffi::export]
impl CompiledDocument {
    /// Export the document, optionally restricted to a range of pages.
    #[uniffi::method(default(pages = None))]
    pub fn export(
        &self,
        format: ExportFormat,
        pages: Option<PageRange>,
    ) -> Result<Vec<u8>, OicanaError> {
        Ok(self.handle.export(format.into(), pages.map(Into::into))?)
    }

    /// Sizes of every page, in document order.
    pub fn pages(&self) -> Vec<PageSize> {
        self.handle.pages().into_iter().map(Into::into).collect()
    }

    /// Warnings of the compilation, if any.
    pub fn warnings(&self) -> Option<String> {
        self.warnings.clone()
    }
}

impl Drop for CompiledDocument {
    fn drop(&mut self) {
        oicana_ffi_core::remove_document(&self.id);
    }
}
