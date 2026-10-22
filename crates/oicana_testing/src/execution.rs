use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use image::{GenericImageView, ImageError};
use json_schema_ast::SchemaDocument;
use json_schema_fuzz::{GenerationConfig, ValueGenerator};
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
            let Some(schema_path) = fuzzed_json_input.schema.clone() else {
                return Err(TestExecutionError::FuzzingSetup(format!(
                    "Fuzzed json input '{}' has no schema configured in the manifest!",
                    fuzzed_input.key
                )));
            };
            let schema = {
                let schema_file = match File::open(self.template_path.join(&schema_path)) {
                    Ok(file) => file,
                    Err(error) => {
                        return Err(TestExecutionError::FuzzingSetup(format!(
                            "Failed to open schema file '{}': {}",
                            schema_path, error
                        )));
                    }
                };
                let schema: serde_json::Value = match serde_json::from_reader(schema_file) {
                    Ok(schema) => schema,
                    Err(error) => {
                        return Err(TestExecutionError::FuzzingSetup(format!(
                            "Failed to parse schema file '{}': {}",
                            schema_path, error
                        )));
                    }
                };
                if let Some(uri) = schema.get("$schema").and_then(serde_json::Value::as_str) {
                    if !uri.contains("/draft/2020-12/") {
                        return Err(TestExecutionError::FuzzingSetup(format!(
                            "Fuzzing requires JSON Schema 2020-12, but the schema '{schema_path}' \
                             declares '{uri}'."
                        )));
                    }
                }
                match SchemaDocument::from_json(&schema) {
                    Ok(schema) => schema,
                    Err(error) => {
                        return Err(TestExecutionError::FuzzingSetup(format!(
                            "Failed to build schema '{}': {}",
                            schema_path, error
                        )));
                    }
                }
            };

            let mut rng = rand::rng();
            let config = GenerationConfig::new(u8::MAX);

            return (0..fuzzed_input.samples)
                .map(|_| {
                    let value =
                        ValueGenerator::generate(&schema, config, &mut rng).map_err(|error| {
                            TestExecutionError::FuzzingSetup(format!(
                                "Failed to generate a value for '{}' from schema '{}': {}",
                                fuzzed_input.key, schema_path, error
                            ))
                        })?;
                    let mut inputs = test.inputs.clone();
                    inputs.with_input(JsonInput::new(fuzzed_input.key.clone(), value.to_string()));
                    debug!("Fuzzing the input '{}' with {}", fuzzed_input.key, value);

                    self.run_with_inputs(inputs, &test.snapshot, test.pdf)
                        .map_err(|error| TestExecutionError::FuzzedInput {
                            key: fuzzed_input.key.clone(),
                            value: value.to_string(),
                            error: Box::new(error),
                        })
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
        let mut warnings: Vec<String> = warnings
            .into_iter()
            .map(|warning| warning.formatted)
            .collect();

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
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(path, optimize_png(&image)?)?;
                    remove_comparison(path)?;
                }
                SnapshotMode::Compare => {
                    return Err(TestExecutionError::SnapshotMissing(path.clone()))
                }
            },
            Snapshot::Some(path, mode) => {
                if compare_images(path, &image, 1)? {
                    remove_comparison(path)?;
                } else {
                    match mode {
                        SnapshotMode::Compare => {
                            match comparison_path(path) {
                                Some(comparison) => fs::write(comparison, optimize_png(&image)?)?,
                                None => error!("Snapshot file had no file stem!"),
                            }
                            return Err(TestExecutionError::SnapshotMismatch(path.clone()));
                        }
                        SnapshotMode::Update => {
                            warnings.push(format!("Overwriting snapshot file at '{path:?}'."));
                            fs::write(path, optimize_png(&image)?)?;
                            remove_comparison(path)?;
                        }
                    }
                }
            }
            Snapshot::None => (),
        }

        Ok(warnings)
    }
}

/// The file a mismatching snapshot is written to.
pub fn comparison_path(snapshot: &Path) -> Option<PathBuf> {
    let mut name = snapshot.file_stem()?.to_os_string();
    name.push(".compare.png");
    Some(snapshot.with_file_name(name))
}

/// Remove a comparison image left over from an earlier mismatch.
fn remove_comparison(snapshot: &Path) -> io::Result<()> {
    let Some(comparison) = comparison_path(snapshot) else {
        return Ok(());
    };
    match fs::remove_file(comparison) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
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
    use crate::{Snapshot, SnapshotMode, Test};
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

    fn run_snapshot(
        dir: &TempDir,
        body: &str,
        snapshot: Snapshot,
    ) -> Result<Vec<String>, TestExecutionError> {
        fs::write(dir.path().join("main.typ"), body).unwrap();
        let manifest_content = fs::read_to_string(dir.path().join("typst.toml")).unwrap();
        let manifest = TemplateManifest::from_toml(&manifest_content).unwrap();
        let mut runner = TestRunnerContext::new()
            .unwrap()
            .get_runner(dir.path(), &manifest)
            .unwrap();

        runner.run(Test {
            inputs: TemplateInputs::new(),
            fuzzed_inputs: None,
            name: "snapshot".to_owned(),
            snapshot,
            pdf: false,
            collection: dir.path().to_path_buf(),
            descriptor: "snapshot".to_owned(),
        })
    }

    #[test]
    fn removes_the_comparison_image_once_the_snapshot_matches() {
        let dir = ua_1_template("");
        let snapshot = dir.path().join("snapshots/nested/test.png");
        let comparison = dir.path().join("snapshots/nested/test.compare.png");

        run_snapshot(
            &dir,
            "A",
            Snapshot::Missing(snapshot.clone(), SnapshotMode::Update),
        )
        .unwrap();
        assert!(snapshot.exists());

        let error = run_snapshot(
            &dir,
            "B",
            Snapshot::Some(snapshot.clone(), SnapshotMode::Compare),
        )
        .unwrap_err();
        assert!(
            matches!(error, TestExecutionError::SnapshotMismatch(_)),
            "{error}"
        );
        assert!(comparison.exists());

        run_snapshot(
            &dir,
            "B",
            Snapshot::Some(snapshot.clone(), SnapshotMode::Update),
        )
        .unwrap();
        assert!(!comparison.exists());

        run_snapshot(
            &dir,
            "A",
            Snapshot::Some(snapshot.clone(), SnapshotMode::Compare),
        )
        .unwrap_err();
        assert!(comparison.exists());
        run_snapshot(&dir, "B", Snapshot::Some(snapshot, SnapshotMode::Compare)).unwrap();
        assert!(!comparison.exists());
    }

    #[test]
    fn fuzzing_rejects_schemas_other_than_2020_12() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("typst.toml"),
            r#"
            [package]
            name = "fuzzed"
            version = "0.1.0"
            entrypoint = "main.typ"

            [tool.oicana]
            manifest_version = 1

            [[tool.oicana.inputs]]
            type = "json"
            key = "data"
            schema = "data.schema.json"
            "#,
        )
        .unwrap();
        fs::write(
            dir.path().join("data.schema.json"),
            r#"{"$schema": "http://json-schema.org/draft-07/schema#", "type": "boolean"}"#,
        )
        .unwrap();
        fs::write(dir.path().join("main.typ"), "Hello").unwrap();
        let manifest_content = fs::read_to_string(dir.path().join("typst.toml")).unwrap();
        let manifest = TemplateManifest::from_toml(&manifest_content).unwrap();
        let mut runner = TestRunnerContext::new()
            .unwrap()
            .get_runner(dir.path(), &manifest)
            .unwrap();

        let error = runner
            .run(Test {
                inputs: TemplateInputs::new(),
                fuzzed_inputs: Some(crate::FuzzJson {
                    samples: 1,
                    key: "data".to_owned(),
                }),
                name: "fuzzed".to_owned(),
                snapshot: Snapshot::None,
                pdf: false,
                collection: dir.path().to_path_buf(),
                descriptor: "fuzzed".to_owned(),
            })
            .unwrap_err();

        let message = error.to_string();
        assert!(
            message.contains("Fuzzing requires JSON Schema 2020-12")
                && message.contains("'http://json-schema.org/draft-07/schema#'"),
            "{message}"
        );
    }

    #[test]
    fn names_the_fuzzed_value_of_a_failing_sample() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("typst.toml"),
            r#"
            [package]
            name = "fuzzed"
            version = "0.1.0"
            entrypoint = "main.typ"

            [tool.oicana]
            manifest_version = 1

            [[tool.oicana.inputs]]
            type = "json"
            key = "data"
            schema = "data.schema.json"
            "#,
        )
        .unwrap();
        fs::write(
            dir.path().join("data.schema.json"),
            r#"{"type": "boolean"}"#,
        )
        .unwrap();
        fs::write(dir.path().join("main.typ"), "#panic(\"always fails\")").unwrap();
        let manifest_content = fs::read_to_string(dir.path().join("typst.toml")).unwrap();
        let manifest = TemplateManifest::from_toml(&manifest_content).unwrap();
        let mut runner = TestRunnerContext::new()
            .unwrap()
            .get_runner(dir.path(), &manifest)
            .unwrap();

        let error = runner
            .run(Test {
                inputs: TemplateInputs::new(),
                fuzzed_inputs: Some(crate::FuzzJson {
                    samples: 1,
                    key: "data".to_owned(),
                }),
                name: "fuzzed".to_owned(),
                snapshot: Snapshot::None,
                pdf: false,
                collection: dir.path().to_path_buf(),
                descriptor: "fuzzed".to_owned(),
            })
            .unwrap_err();

        let TestExecutionError::FuzzedInput {
            ref key, ref value, ..
        } = error
        else {
            panic!("Unexpected error: {error}");
        };
        assert_eq!(key, "data");
        assert!(value == "true" || value == "false", "{value}");
        let message = error.to_string();
        assert!(message.contains("always fails"), "{message}");
        assert!(
            message.contains(&format!("The fuzzed input 'data' had the value: {value}")),
            "{message}"
        );
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
    /// The test failed for a generated value of a fuzzed input
    #[error("{error}\nThe fuzzed input '{key}' had the value: {value}")]
    FuzzedInput {
        /// The key of the fuzzed input
        key: String,
        /// The generated JSON value
        value: String,
        /// Why the test failed
        error: Box<TestExecutionError>,
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
