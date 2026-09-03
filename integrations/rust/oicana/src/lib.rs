//! # Oicana
//!
//! _Generate PDFs from Typst templates, in process._
//!
//! Oicana compiles [Typst](https://typst.app/) templates to PDF, PNG, and SVG. Documents are
//! designed as templates that declare typed inputs; your application loads a packed template
//! once and renders it from JSON. No headless browser, no hand-coded layout.
//!
//! ```no_run
//! use std::fs::File;
//!
//! use oicana::Template;
//! use oicana::export::pdf::export_pdf;
//! use oicana::input::input::json::JsonInput;
//! use oicana::input::{CompilationConfig, TemplateInputs};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut template = Template::init(File::open("invoice-0.1.0.zip")?)?;
//!
//! let mut inputs = TemplateInputs::new();
//! inputs.with_config(CompilationConfig::production());
//! inputs.with_input(JsonInput::new("invoice", r#"{"number": "2026-001"}"#));
//!
//! let document = template.compile(inputs)?;
//!
//! let pdf = export_pdf(
//!     &document.document,
//!     &template,
//!     template.manifest().pdf_standards(),
//!     template.manifest().pdf_tagged(),
//!     None,
//! )?;
//! # Ok(())
//! # }
//! ```
//!
//! [`Template::compile`] returns a reusable [`CompiledDocument`], and the free functions in
//! [`export`] turn it into bytes. Keeping the two apart lets one compilation produce several
//! formats or page ranges.
//!
//! [`Template::init`] only reads the template, it does not compile it, so the first
//! [`Template::compile`] call pays the full compilation cost.
//!
//! # Feature flags
//!
//! `pdf` and `packed` are enabled by default. Enable `png` or `svg` for the other export
//! formats, `native` to read an unpacked template from disk, and `preloaded` to read one from
//! an in-memory file map in tests.
//!
//! # Where to go next
//!
//! The [documentation](https://oicana.com/docs) covers templates, inputs, and the CLI that packs
//! them; the [Axum example](https://github.com/oicana/oicana-example-rust-axum) showcases a more complete
//! service.

#[cfg(feature = "packed")]
use std::io::{Read, Seek};

use ::typst::{
    diag::{FileResult, SourceDiagnostic},
    ecow::EcoVec,
    foundations::Bytes,
    syntax::{FileId, Source},
};
#[cfg(feature = "packed")]
use oicana_files::packed::{PackedTemplate, PackedTemplateError, ZipLimits};
use oicana_files::TemplateFiles;
use oicana_input::TemplateInputs;
use oicana_template::manifest::TemplateManifest;
#[cfg(feature = "packed")]
use oicana_world::manifest::OicanaWorldFiles;
use oicana_world::{
    diagnostics::{DiagnosticColor, TemplateDiagnostics},
    fonts::FontSource,
    manifest::OicanaWorldManifestError,
    world::{OicanaWorld, WorldCreationError},
    CompiledDocument, InputValidationError, TemplateCompilationFailure,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;

/// File-source implementations for Oicana templates.
pub mod files {
    pub use oicana_files::TemplateFiles;

    #[cfg(feature = "native")]
    pub use oicana_files::native;
    #[cfg(feature = "packed")]
    pub use oicana_files::packed;
    #[cfg(feature = "preloaded")]
    pub use oicana_files::preloaded;
}

/// Template inputs and compilation configuration.
pub use oicana_input as input;

/// Oicana world, diagnostics, and compilation primitives.
pub use oicana_world as world;

/// Fonts a host can make available to templates.
pub use oicana_world::fonts;

/// Template manifest and configuration types.
pub use oicana_template as template;

/// Export helpers for compiled documents (PDF, PNG, SVG).
#[cfg(any(feature = "pdf", feature = "png", feature = "svg"))]
pub use oicana_export as export;

/// Re-exports of Typst types that appear in this crate's public API.
pub mod typst {
    pub use ::typst::diag::{FileResult, SourceDiagnostic};
    pub use ::typst::ecow::EcoVec;
    pub use ::typst::foundations::{Array, Bytes, Dict, IntoValue, Str, Value};
    pub use ::typst::syntax::{FileId, Source};
}

/// Global cache age configuration.
///
/// Default is 10, meaning cache entries used during the last 10 eviction cycles are kept.
/// usize::MAX is used internally to represent disabled eviction.
static CACHE_EVICTION_AGE: AtomicUsize = AtomicUsize::new(10);

/// Configure automatic cache eviction after each compilation.
///
/// # Parameters
///
/// `max_age` (start value: 10) - Maximum age threshold, or null to disable:
///   - `null` - Disables cache eviction (cache never cleared)
///   - `0` - Clears all cache entries with every eviction
///   - `1` - Keeps only entries used since the last eviction
///   - `n` - Keeps entries used within the last n evictions
pub fn configure_automatic_cache_eviction(max_age: Option<usize>) {
    CACHE_EVICTION_AGE.store(max_age.unwrap_or(usize::MAX), Ordering::Relaxed);
}

// Re-export evict_cache from oicana_world for convenience
pub use oicana_world::evict_cache;

/// Support for native Oicana templates.
/// Native templates are not packed. They are a Typst project in a native file system.
#[cfg(feature = "native")]
pub mod native;

/// A prepared Oicana Template
pub struct Template<F: TemplateFiles> {
    world: OicanaWorld<F>,
}

#[cfg(feature = "packed")]
impl Template<PackedTemplate> {
    /// Initialize the given template
    pub fn init<R: Read + Seek>(template: R) -> Result<Self, TemplateInitializationError> {
        Self::init_with_fonts(template, &[])
    }

    /// Initialize the given template with additional fonts.
    ///
    /// The fonts are available to the template on top of the ones packed with
    /// it. Declare required font families under `tool.oicana.fonts.require`
    /// in the manifest to have template initialization check font availability.
    pub fn init_with_fonts<R: Read + Seek>(
        template: R,
        fonts: &[FontSource],
    ) -> Result<Self, TemplateInitializationError> {
        Self::init_with_fonts_and_limits(template, fonts, ZipLimits::default())
    }

    /// Initialize the given template, reading its archive with the given limits.
    ///
    /// See [`ZipLimits`] for the defaults that [`Template::init`] applies.
    pub fn init_with_limits<R: Read + Seek>(
        template: R,
        limits: ZipLimits,
    ) -> Result<Self, TemplateInitializationError> {
        Self::init_with_fonts_and_limits(template, &[], limits)
    }

    /// Initialize the given template with additional fonts, reading its archive
    /// with the given limits.
    ///
    /// See [`Template::init_with_fonts`] for how the fonts are used and
    /// [`ZipLimits`] for the defaults that [`Template::init`] applies.
    pub fn init_with_fonts_and_limits<R: Read + Seek>(
        template: R,
        fonts: &[FontSource],
        limits: ZipLimits,
    ) -> Result<Self, TemplateInitializationError> {
        let files = PackedTemplate::new_with_limits(template, limits)?;
        let manifest = files.manifest()?;

        let world = OicanaWorld::new_with_fonts(files, TemplateInputs::new(), manifest, fonts)?;

        Ok(Template { world })
    }
}

impl<Files: TemplateFiles> Template<Files> {
    /// Compile the template with given inputs
    pub fn compile(&mut self, inputs: TemplateInputs) -> Result<CompiledDocument, CompileError> {
        self.world.update_inputs(inputs)?;
        let result = self.world.compile()?;
        let cache_age = CACHE_EVICTION_AGE.load(Ordering::Relaxed);
        if cache_age != usize::MAX {
            oicana_world::evict_cache(cache_age);
        }
        Ok(result)
    }

    /// Get the manifest of the template
    pub fn manifest(&self) -> &TemplateManifest {
        self.world.manifest()
    }

    /// Return the source of a file in the template project
    pub fn source(&self, id: FileId) -> FileResult<Source> {
        self.world.files.source(id)
    }

    /// Return a file in the template project as bytes
    pub fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.world.files.file(id)
    }

    /// Configure the coloring of diagnostic output from this template
    pub fn set_diagnostic_color(&mut self, color: DiagnosticColor) {
        self.world.color = color;
    }

    /// Enable or disable JSON schema validation for this template.
    ///
    /// When enabled (the default), JSON inputs are validated against their schemas
    /// before compilation.
    pub fn set_validate_inputs(&mut self, validate: bool) {
        self.world.validate_inputs = validate;
    }
}

impl<Files: TemplateFiles> TemplateDiagnostics for Template<Files> {
    fn format_diagnostics(&self, diagnostics: EcoVec<SourceDiagnostic>) -> Vec<u8> {
        self.world.format_diagnostics(diagnostics)
    }
}

/// An error occurred while initiating the template
#[derive(Error, Debug)]
pub enum TemplateInitializationError {
    /// An error concerning the template manifest
    #[error(transparent)]
    ManifestError(#[from] OicanaWorldManifestError),

    /// Error while creating the template world
    #[error(transparent)]
    WorldCreationError(#[from] WorldCreationError),

    /// The packed template could not be read
    #[cfg(feature = "packed")]
    #[error(transparent)]
    PackedTemplateError(#[from] PackedTemplateError),

    /// The data directory for Typst packages could not be found
    #[error("Failed to find the data directory for Typst packages on the System")]
    PackageDirectoryNotFound,
}

/// An error that occurred during template compilation
#[derive(Error, Debug)]
pub enum CompileError {
    /// A JSON input failed schema validation
    #[error(transparent)]
    ValidationFailed(#[from] InputValidationError),

    /// The Typst compilation failed
    #[error(transparent)]
    CompilationFailed(#[from] TemplateCompilationFailure),
}
