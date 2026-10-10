use std::collections::HashMap;

use oicana_ffi_core::BlobWithMetadata;

/// How required inputs without a value are resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CompilationMode {
    /// Fall back to the development value, then the default value.
    Development,
    /// Fall back to the default value.
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

/// Coloring of compilation diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DiagnosticColor {
    /// Plain text.
    None,
    /// ANSI escape codes.
    Ansi,
}

impl From<DiagnosticColor> for oicana_ffi_core::DiagnosticColor {
    fn from(color: DiagnosticColor) -> Self {
        match color {
            DiagnosticColor::None => oicana_ffi_core::DiagnosticColor::None,
            DiagnosticColor::Ansi => oicana_ffi_core::DiagnosticColor::Ansi,
        }
    }
}

/// Format of an exported document.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum ExportFormat {
    /// PDF, using the standards from the template manifest.
    Pdf,
    /// PNG image.
    Png {
        /// Pixels per typographic point.
        pixels_per_pt: f32,
    },
    /// SVG image.
    Svg,
}

impl From<ExportFormat> for oicana_ffi_core::ExportFormat {
    fn from(format: ExportFormat) -> Self {
        match format {
            ExportFormat::Pdf => oicana_ffi_core::ExportFormat::Pdf,
            ExportFormat::Png { pixels_per_pt } => {
                oicana_ffi_core::ExportFormat::Png { pixels_per_pt }
            }
            ExportFormat::Svg => oicana_ffi_core::ExportFormat::Svg,
        }
    }
}

/// A contiguous range of pages, with 0-based inclusive bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct PageRange {
    /// First page to export. `None` starts at the first page.
    #[uniffi(default = None)]
    pub start: Option<u64>,
    /// Last page to export. `None` ends at the last page.
    #[uniffi(default = None)]
    pub end: Option<u64>,
}

impl From<PageRange> for oicana_ffi_core::PageRange {
    fn from(range: PageRange) -> Self {
        oicana_ffi_core::PageRange {
            start: range.start.map(saturating_usize),
            end: range.end.map(saturating_usize),
        }
    }
}

/// Limits for reading a packed template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ZipLimits {
    /// Maximum number of zip entries. `None` keeps the default.
    #[uniffi(default = None)]
    pub max_entries: Option<u64>,
    /// Maximum total decompressed size in bytes. `None` keeps the default.
    #[uniffi(default = None)]
    pub max_total_decompressed_bytes: Option<u64>,
}

impl From<ZipLimits> for Option<oicana_ffi_core::ZipLimits> {
    fn from(limits: ZipLimits) -> Self {
        oicana_ffi_core::ZipLimits::from_optional(
            limits.max_entries,
            limits.max_total_decompressed_bytes,
        )
    }
}

/// A blob input value.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BlobInput {
    /// Raw bytes of the blob.
    pub data: Vec<u8>,
    /// Metadata of the blob as a JSON object.
    #[uniffi(default = "{}")]
    pub metadata: String,
}

/// Size of a page in typographic points.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct PageSize {
    /// Page width.
    pub width: f64,
    /// Page height.
    pub height: f64,
}

impl From<oicana_ffi_core::PageSize> for PageSize {
    fn from(size: oicana_ffi_core::PageSize) -> Self {
        PageSize {
            width: size.width,
            height: size.height,
        }
    }
}

/// A font face registered by the host.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RegisteredFont {
    /// The family name.
    pub family: String,
    /// The file the face was read from, if it was registered by path.
    pub path: Option<String>,
}

impl From<oicana_ffi_core::RegisteredFont> for RegisteredFont {
    fn from(font: oicana_ffi_core::RegisteredFont) -> Self {
        RegisteredFont {
            family: font.family,
            path: font.path,
        }
    }
}

/// An exported document with the warnings of its compilation.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ExportOnceResult {
    /// The exported document.
    pub bytes: Vec<u8>,
    /// Compilation warnings, if any.
    pub warnings: Option<String>,
}

pub(crate) fn core_blobs(blobs: HashMap<String, BlobInput>) -> HashMap<String, BlobWithMetadata> {
    blobs
        .into_iter()
        .map(|(key, blob)| {
            (
                key,
                BlobWithMetadata {
                    bytes: blob.data,
                    meta: blob.metadata,
                },
            )
        })
        .collect()
}

pub(crate) fn saturating_usize(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}
