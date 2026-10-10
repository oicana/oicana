use oicana_ffi_core::FfiError;

/// Errors raised by Oicana.
///
/// Each variant carries the human-readable message, including rendered compilation
/// diagnostics.
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum OicanaError {
    /// The packed template could not be read or its manifest is invalid.
    #[error("{0}")]
    InvalidTemplate(String),
    /// An input value, its blob metadata, or a page range is invalid.
    #[error("{0}")]
    InvalidInput(String),
    /// Compiling the template failed.
    #[error("{0}")]
    Compilation(String),
    /// A file requested from the template could not be loaded.
    #[error("{0}")]
    FileNotFound(String),
    /// Encoding the document to the requested format failed.
    #[error("{0}")]
    Export(String),
    /// A template or document is no longer registered.
    #[error("{0}")]
    NotFound(String),
    /// An unexpected internal failure.
    #[error("{0}")]
    Internal(String),
}

impl From<FfiError> for OicanaError {
    fn from(error: FfiError) -> Self {
        let message = error.to_string();
        match error {
            FfiError::PackedTemplate(_) | FfiError::Manifest(_) | FfiError::WorldCreation(_) => {
                OicanaError::InvalidTemplate(message)
            }
            FfiError::InputValidation(_)
            | FfiError::BlobMetadata { .. }
            | FfiError::PageRangeParse(_) => OicanaError::InvalidInput(message),
            FfiError::Compilation(_) | FfiError::WarmUp(_) => OicanaError::Compilation(message),
            FfiError::SourceLoad { .. } | FfiError::FileLoad { .. } => {
                OicanaError::FileNotFound(message)
            }
            FfiError::Export { .. } => OicanaError::Export(message),
            FfiError::TemplateNotRegistered(_)
            | FfiError::DocumentNotFound(_)
            | FfiError::InvalidDocumentId(_) => OicanaError::NotFound(message),
            FfiError::ManifestSerialization(_)
            | FfiError::PageSizesSerialization(_)
            | FfiError::ExportFormatParse(_) => OicanaError::Internal(message),
        }
    }
}
