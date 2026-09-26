use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use image::{GenericImageView, ImageError};
use log::{debug, error};
use oicana::{CompileError, Template, TemplateInitializationError};
use oicana_export::pdf::export_pdf;
use oicana_export::png::{export_png, PngExportError};
use oicana_files::native::{package_data_dir, NativeTemplate};
use oicana_input::{InputDefinition, JsonInput, TemplateInputs};
use oicana_template::manifest::TemplateManifest;
use oicana_world::diagnostics::DiagnosticColor;
use oicana_world::fonts::FontSource;
use oicana_world::CompiledDocument;
use oxipng::PngError;
use rand::thread_rng;
use thiserror::Error;

use crate::{Snapshot, SnapshotMode, Test};

/// Context for test runners
pub struct TestRunnerContext {
    packages: PathBuf,
    fonts: Vec<FontSource>,
}

impl TestRunnerContext {
    /// Create a new test runner
    pub fn new() -> Result<Self, CreateTestRunnerError> {
        Self::with_fonts(vec![])
    }

    /// Create a new test runner with additional fonts.
    ///
    /// The fonts are available to every template the context runs, mirroring a
    /// host that registers them at runtime.
    pub fn with_fonts(fonts: Vec<FontSource>) -> Result<Self, CreateTestRunnerError> {
        let packages = package_data_dir().ok_or(CreateTestRunnerError::NoPackageDirectory)?;

        Ok(TestRunnerContext { packages, fonts })
    }

    /// Prepare a runner for the template at the given path
    pub fn get_runner(
        &self,
        path: &Path,
        manifest: &TemplateManifest,
    ) -> Result<TestRunner, TemplateInitializationError> {
        Ok(TestRunner {
            instance: Template::<NativeTemplate>::from_with_fonts(
                path,
                &self.packages,
                manifest.clone(),
                &self.fonts,
            )?,
            template_path: path.to_path_buf(),
        })
    }
}

/// Execution context for tests of a single template
pub struct TestRunner {
    instance: Template<NativeTemplate>,
    template_path: PathBuf,
}

impl TestRunner {
    /// Configure the coloring of diagnostic output from this template.
    pub fn set_diagnostic_color(&mut self, color: DiagnosticColor) {
        self.instance.set_diagnostic_color(color);
    }

    /// Reset file access tracking for the next compilation cycle.
    ///
    /// This clears the accessed flags on file slots but preserves cached data
    /// and fingerprints, enabling efficient incremental recompilation.
    pub fn reset(&mut self) {
        self.instance.reset();
    }

    /// Return system paths of all files accessed during test compilations.
    pub fn dependencies(&self) -> Vec<PathBuf> {
        self.instance.dependencies()
    }

    /// Run the test case
    pub fn run(&mut self, test: Test) -> Result<Vec<String>, TestExecutionError> {
        if let Some(fuzzed_input) = test.fuzzed_inputs {
            let fuzzed_json_input = self
                .instance
                .manifest()
                .tool
                .oicana
                .inputs
                .iter()
                .filter_map(|input| {
                    let InputDefinition::Json(json) = input else {
                        return None;
                    };
                    Some(json)
                })
                .find(|input| input.key == fuzzed_input.key);
            let Some(fuzzed_json_input) = fuzzed_json_input else {
                return Err(TestExecutionError::FuzzingSetup(format!(
                    "Fuzzed input '{}' has no input definition in the manifest!",
                    fuzzed_input.key
                )));
            };
            let Some(schema_path) = &fuzzed_json_input.schema else {
                return Err(TestExecutionError::FuzzingSetup(format!(
                    "Fuzzed json input '{}' has no schema configured in the manifest!",
                    fuzzed_input.key
                )));
            };
            let schema = {
                let schema_file = match File::open(self.template_path.join(schema_path)) {
                    Ok(file) => file,
                    Err(error) => {
                        return Err(TestExecutionError::FuzzingSetup(format!(
                            "Failed to open schema file '{}': {}",
                            schema_path, error
                        )));
                    }
                };
                let schema = match serde_json::from_reader(schema_file) {
                    Ok(schema) => schema,
                    Err(error) => {
                        return Err(TestExecutionError::FuzzingSetup(format!(
                            "Failed to parse schema file '{}': {}",
                            schema_path, error
                        )));
                    }
                };
                match json_schema_ast::build_and_resolve_schema(&schema) {
                    Ok(schema) => schema,
                    Err(error) => {
                        return Err(TestExecutionError::FuzzingSetup(format!(
                            "Failed to build schema '{}': {}",
                            schema_path, error
                        )));
                    }
                }
            };

            let mut rng = thread_rng();

            return (0..fuzzed_input.samples)
                .map(|_| {
                    let value = json_schema_fuzz::generate_value(&schema, &mut rng, u8::MAX);
                    let mut inputs = test.inputs.clone();
                    inputs.with_input(JsonInput::new(fuzzed_input.key.clone(), value.to_string()));
                    debug!("Fuzzing the input '{}' with {}", fuzzed_input.key, value);

                    self.run_with_inputs(inputs, &test.snapshot, test.pdf)
                })
                .map(|res| res.map(|vec| vec.into_iter()))
                .collect::<Result<Vec<_>, _>>()
                .map(|iterators| iterators.into_iter().flatten().collect());
        }

        self.run_with_inputs(test.inputs, &test.snapshot, test.pdf)
    }

    fn run_with_inputs(
        &mut self,
        inputs: TemplateInputs,
        snapshot: &Snapshot,
        pdf: bool,
    ) -> Result<Vec<String>, TestExecutionError> {
        let CompiledDocument { document, warnings } = self.instance.compile(inputs)?;
        let mut warnings = if let Some(warning) = warnings {
            vec![warning]
        } else {
            vec![]
        };

        if pdf {
            let standards = self.instance.manifest().pdf_standards().to_vec();
            let tagged = self.instance.manifest().pdf_tagged();
            export_pdf(&document, &self.instance, &standards, tagged, None).map_err(
                |diagnostics| TestExecutionError::PdfExport {
                    standards: standards
                        .iter()
                        .map(|standard| standard.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                    diagnostics: diagnostics.to_string(),
                },
            )?;
        }

        let image = export_png(&document, 1., None)?;
        match snapshot {
            Snapshot::Missing(path, mode) => match mode {
                SnapshotMode::Update => {
                    warnings.push(format!(
                        "Writing snapshot file at {path:?}, because it was missing"
                    ));
                    fs::write(path, optimize_png(&image)?)?;
                }
                SnapshotMode::Compare => {
                    return Err(TestExecutionError::SnapshotMissing(path.clone()))
                }
            },
            Snapshot::Some(path, mode) => {
                if !compare_images(path, &image, 1)? {
                    let mut compare_path = path.clone();
                    if let Some(stem) = compare_path.file_stem() {
                        let mut new_name = stem.to_os_string();
                        match mode {
                            SnapshotMode::Compare => {
                                new_name.push(".compare.png");
                            }
                            SnapshotMode::Update => {
                                new_name.push(".png");
                                warnings.push(format!("Overwriting snapshot file at '{path:?}'."));
                            }
                        }
                        compare_path.set_file_name(new_name);
                        fs::write(compare_path, optimize_png(&image)?)?;
                    } else {
                        error!("Snapshot file had no file stem!");
                    }
                    if matches!(mode, SnapshotMode::Compare) {
                        return Err(TestExecutionError::SnapshotMismatch(path.clone()));
                    }
                }
            }
            Snapshot::None => (),
        }

        Ok(warnings)
    }
}

/// Losslessly recompress a PNG byte buffer at oxipng's strongest preset before
/// it lands on disk as a snapshot.
fn optimize_png(image: &[u8]) -> Result<Vec<u8>, PngError> {
    oxipng::optimize_from_memory(image, &oxipng::Options::max_compression())
}

/// Compare the image file at `path` with the given data. If there is a pixel value with a higher difference
/// than `tolerance`, return `false`.
pub fn compare_images(path: &Path, data: &[u8], tolerance: u8) -> Result<bool, ImageError> {
    let img1 = image::open(path)?;
    let img2 = image::load_from_memory(data)?;

    if img1.dimensions() != img2.dimensions() {
        return Ok(false);
    }

    let pixels1 = img1.pixels();
    let pixels2 = img2.pixels();

    for ((_, _, p1), (_, _, p2)) in pixels1.zip(pixels2) {
        let diff =
            p1.0.iter()
                .zip(p2.0.iter())
                .map(|(a, b)| (*a as i16 - *b as i16).unsigned_abs() as u8)
                .max()
                .unwrap_or(0);

        if diff > tolerance {
            return Ok(false);
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Snapshot, Test};
    use std::fs;
    use tempfile::{tempdir, TempDir};

    /// Write a template that enforces PDF/UA-1 and renders the given body.
    fn ua_1_template(body: &str) -> TempDir {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("typst.toml"),
            r#"
            [package]
            name = "ua-1-template"
            version = "0.1.0"
            entrypoint = "main.typ"

            [tool.oicana]
            manifest_version = 1

            [tool.oicana.export.pdf]
            standards = ["ua-1"]
            "#,
        )
        .unwrap();
        fs::write(dir.path().join("main.typ"), body).unwrap();

        dir
    }

    fn run_template(dir: &TempDir, pdf: bool) -> Result<Vec<String>, TestExecutionError> {
        let manifest_content = fs::read_to_string(dir.path().join("typst.toml")).unwrap();
        let manifest = TemplateManifest::from_toml(&manifest_content).unwrap();
        let mut runner = TestRunnerContext::new()
            .unwrap()
            .get_runner(dir.path(), &manifest)
            .unwrap();

        runner.run(Test {
            inputs: TemplateInputs::new(),
            fuzzed_inputs: None,
            name: "pdf-standards".to_owned(),
            snapshot: Snapshot::None,
            pdf,
            collection: dir.path().to_path_buf(),
            descriptor: "pdf-standards".to_owned(),
        })
    }

    #[test]
    fn fails_for_document_violating_the_pdf_standard() {
        let dir = ua_1_template("Hello");

        let error = run_template(&dir, true).unwrap_err();

        // PDF/UA-1 requires a document title.
        assert!(
            matches!(error, TestExecutionError::PdfExport { ref standards, .. } if standards == "ua-1"),
            "Unexpected error: {error}"
        );
    }

    #[test]
    fn passes_for_document_conforming_to_the_pdf_standard() {
        let dir = ua_1_template("#set document(title: \"Accessible\")\nHello");

        run_template(&dir, true).expect("Test should pass for a conforming document");
    }

    #[test]
    fn skips_pdf_export_when_disabled() {
        let dir = ua_1_template("Hello");

        run_template(&dir, false).expect("Test should pass when the PDF export is disabled");
    }
}

/// Errors that can be produced when executing tests
#[derive(Debug, Error)]
pub enum CreateTestRunnerError {
    /// Could not get the local Typst package directory
    #[error("Failed to get Typst package directory.")]
    NoPackageDirectory,
}

/// Errors that can be produced when executing tests
#[derive(Debug, Error)]
pub enum TestExecutionError {
    /// A test failed to compile or validate
    #[error("{0}")]
    CompileError(#[from] CompileError),
    /// Failed to export png image
    #[error("{0}")]
    ExportError(#[from] PngExportError),
    /// The document does not conform to the PDF standards of the template manifest
    #[error("Failed to export PDF for the standards '{standards}' configured in the template manifest:\n{diagnostics}")]
    PdfExport {
        /// The PDF standards that the export was attempted with
        standards: String,
        /// Diagnostics of the failed export
        diagnostics: String,
    },
    /// Failure during fuzzing setup
    #[error("{0}")]
    FuzzingSetup(String),
    /// Failed to write or read snapshot image
    #[error("Failed to write or read snapshot image: {0}")]
    Io(#[from] io::Error),
    /// Failed to compare the snapshot
    #[error("Failed to compare the snapshot: {0}")]
    Comparison(#[from] ImageError),
    /// Failed to optimize the snapshot PNG bytes before writing
    #[error("Failed to optimize snapshot PNG: {0}")]
    PngOptimization(#[from] PngError),
    /// The snapshot does not match the result. Rerun with `--update`/`-u` to update it.
    #[error("The snapshot '{0}' does not match the result. Rerun with --update/-u to update it.")]
    SnapshotMismatch(PathBuf),
    /// The snapshot file is missing. Rerun with `--update`/`-u` to create it.
    #[error("The snapshot file '{0}' is missing. Rerun with --update/-u to create it.")]
    SnapshotMissing(PathBuf),
}
