use crate::target::TargetArgs;
use clap::Args;
use console::{style, Emoji};
use log::info;
use oicana::export::pdf::{validate_pdf_standards, validate_pdf_tagging};
use oicana::input::InputDefinition;
use oicana::template::validate_native_template;
use std::path::Path;

static CHECKMARK: Emoji<'_, '_> = Emoji("✔️", "");

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[clap(flatten)]
    target: TargetArgs,
    #[arg(long, help = "Fail the validation if any warnings are reported")]
    deny_warnings: bool,
}

#[rustfmt::skip]
pub const VALIDATE_AFTER_HELP: &str = color_print::cstr!("\
<s><u>Examples:</></>
  oicana validate templates/invoice
  oicana validate -a
  oicana validate templates -a
  oicana validate -a --deny-warnings
");

pub fn validate(args: ValidateArgs) -> anyhow::Result<()> {
    let templates = args.target.get_targets()?;

    let mut all_passed = true;
    let mut passed_count = 0;
    let mut warning_count = 0;
    let template_count = templates.len();

    for template in templates {
        let validation_result = validate_native_template(&template.path);
        match validation_result {
            Err(e) => {
                eprintln!("Template {:?}: {e}", template.path);
                all_passed = false;
            }
            Ok(manifest) => {
                info!("Template {:?}: manifest valid", template.path);

                let entrypoint = template.path.join(manifest.package.entrypoint.as_str());
                if !entrypoint.exists() {
                    eprintln!(
                        "Template {:?}: entrypoint file '{}' does not exist",
                        template.path, manifest.package.entrypoint
                    );
                    all_passed = false;
                    continue;
                }

                let InputIssues {
                    mut errors,
                    warnings,
                } = validate_input_values(&template.path, &manifest.tool.oicana.inputs);

                let pdf = &manifest.tool.oicana.export.pdf;
                if let Err(error) = validate_pdf_standards(&pdf.standards) {
                    errors.push(error);
                }
                if let Err(error) = validate_pdf_tagging(&pdf.standards, pdf.tagged) {
                    errors.push(error);
                }

                warning_count += warnings.len();
                for warning in &warnings {
                    eprintln!(
                        "{}: Template {:?}: {warning}",
                        style("Warning").yellow().for_stderr(),
                        template.path
                    );
                }

                if errors.is_empty() {
                    info!("Template {:?}: all checks passed", template.path);
                    passed_count += 1;
                    println!(
                        "{CHECKMARK}  {} valid",
                        style(&manifest.package.name).bold(),
                    );
                } else {
                    all_passed = false;
                    for error in &errors {
                        eprintln!("Template {:?}: {error}", template.path);
                    }
                }
            }
        }
    }

    if !all_passed {
        anyhow::bail!("Validation failed for one or more templates.")
    }

    if args.deny_warnings && warning_count > 0 {
        anyhow::bail!("Validation reported warnings and --deny-warnings is set.")
    }

    println!(
        "\nValidated {} template{} successfully{}",
        passed_count,
        if template_count == 1 { "" } else { "s" },
        match warning_count {
            0 => String::new(),
            1 => " with 1 warning".to_owned(),
            count => format!(" with {count} warnings"),
        },
    );

    Ok(())
}

/// Errors and warnings collected while validating a template's inputs.
#[derive(Debug, Default)]
struct InputIssues {
    errors: Vec<String>,
    warnings: Vec<String>,
}

/// Validate the fallback values configured for a template's inputs.
fn validate_input_values(template_path: &Path, inputs: &[InputDefinition]) -> InputIssues {
    let mut issues = InputIssues::default();

    for input in inputs {
        match input {
            InputDefinition::Json(json_def) => {
                let validator = json_def.schema.as_ref().and_then(|schema_path| {
                    compile_schema(
                        template_path,
                        &json_def.key,
                        schema_path,
                        &mut issues.errors,
                    )
                    .map(|validator| (validator, schema_path))
                });

                for (label, file_path) in [
                    ("default", &json_def.default),
                    ("development", &json_def.development),
                ] {
                    let Some(file_path) = file_path else {
                        continue;
                    };

                    let content = match std::fs::read(template_path.join(file_path)) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            issues.errors.push(format!(
                                "Input '{}': failed to read {label} value file '{file_path}': {e}",
                                json_def.key,
                            ));
                            continue;
                        }
                    };

                    let Some((validator, schema_path)) = &validator else {
                        continue;
                    };

                    let parsed: serde_json::Value = match serde_json::from_slice(&content) {
                        Ok(v) => v,
                        Err(e) => {
                            issues.errors.push(format!(
                                "Input '{}': {label} value file '{file_path}' is not valid JSON: {e}",
                                json_def.key,
                            ));
                            continue;
                        }
                    };

                    if !validator.is_valid(&parsed) {
                        let validation_errors: Vec<String> = validator
                            .iter_errors(&parsed)
                            .map(|error| {
                                let path = error.instance_path().to_string();
                                if path.is_empty() {
                                    error.to_string()
                                } else {
                                    format!("  at {path}: {error}")
                                }
                            })
                            .collect();

                        issues.errors.push(format!(
                            "Input '{}': {label} value file '{file_path}' does not match schema '{schema_path}':\n{}",
                            json_def.key,
                            validation_errors.join("\n"),
                        ));
                    }
                }
            }
            InputDefinition::Blob(blob_def) => {
                for (label, fallback) in [
                    ("default", &blob_def.default),
                    ("development", &blob_def.development),
                ] {
                    let Some(fallback) = fallback else {
                        continue;
                    };

                    let file_path = &fallback.file;
                    if let Err(e) = std::fs::metadata(template_path.join(file_path)) {
                        issues.errors.push(format!(
                            "Input '{}': failed to read {label} value file '{file_path}': {e}",
                            blob_def.key,
                        ));
                    }
                }
            }
        }

        if input.required() && !has_fallback(input) {
            issues.warnings.push(format!(
                "Input '{}' is required but has no default or development value. \
                 Compiling without a supplied value for it will fail, including the warm-up \
                 compilation that most integrations run when registering the template. \
                 Set a 'development' value that applies outside of production mode only, \
                 or a 'default' which also applies in production.",
                input.key(),
            ));
        }
    }

    issues
}

/// Whether the input has a `default` or `development` value configured.
fn has_fallback(input: &InputDefinition) -> bool {
    match input {
        InputDefinition::Json(def) => def.default.is_some() || def.development.is_some(),
        InputDefinition::Blob(def) => def.default.is_some() || def.development.is_some(),
    }
}

/// Read and compile the JSON schema of an input, reporting failures as errors.
fn compile_schema(
    template_path: &Path,
    key: &str,
    schema_path: &str,
    errors: &mut Vec<String>,
) -> Option<jsonschema::Validator> {
    let schema_bytes = match std::fs::read(template_path.join(schema_path)) {
        Ok(bytes) => bytes,
        Err(e) => {
            errors.push(format!(
                "Input '{key}': failed to read schema file '{schema_path}': {e}"
            ));
            return None;
        }
    };

    let schema_value: serde_json::Value = match serde_json::from_slice(&schema_bytes) {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!(
                "Input '{key}': failed to parse schema file '{schema_path}': {e}"
            ));
            return None;
        }
    };

    match jsonschema::validator_for(&schema_value) {
        Ok(validator) => Some(validator),
        Err(e) => {
            errors.push(format!(
                "Input '{key}': failed to compile schema '{schema_path}': {e}"
            ));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oicana::input::{BlobInputDefinition, FallbackBlobInput};
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    fn setup_template(
        schema: &str,
        default_value: Option<&str>,
        dev_value: Option<&str>,
    ) -> (tempfile::TempDir, Vec<InputDefinition>) {
        let dir = tempdir().unwrap();

        let mut schema_file = File::create(dir.path().join("data.schema.json")).unwrap();
        write!(schema_file, "{schema}").unwrap();

        if let Some(value) = default_value {
            let mut f = File::create(dir.path().join("default.json")).unwrap();
            write!(f, "{value}").unwrap();
        }

        if let Some(value) = dev_value {
            let mut f = File::create(dir.path().join("dev.json")).unwrap();
            write!(f, "{value}").unwrap();
        }

        let inputs = vec![InputDefinition::Json(oicana::input::JsonInputDefinition {
            key: "data".to_string(),
            required: true,
            default: default_value.map(|_| "default.json".to_string()),
            development: dev_value.map(|_| "dev.json".to_string()),
            schema: Some("data.schema.json".to_string()),
            validate: true,
        })];

        (dir, inputs)
    }

    const SCHEMA: &str = r#"{
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer" }
        },
        "required": ["name"]
    }"#;

    #[test]
    fn valid_default_and_dev_values() {
        let (dir, inputs) = setup_template(
            SCHEMA,
            Some(r#"{"name": "Alice", "age": 30}"#),
            Some(r#"{"name": "Bob"}"#),
        );

        let errors = validate_input_values(dir.path(), &inputs).errors;
        assert!(errors.is_empty(), "Expected no errors, got: {errors:?}");
    }

    #[test]
    fn invalid_default_value_reports_error() {
        let (dir, inputs) = setup_template(
            SCHEMA,
            Some(r#"{"age": 30}"#), // missing required "name"
            None,
        );

        let errors = validate_input_values(dir.path(), &inputs).errors;
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("default"));
        assert!(errors[0].contains("does not match schema"));
    }

    #[test]
    fn invalid_dev_value_reports_error() {
        let (dir, inputs) = setup_template(
            SCHEMA,
            None,
            Some(r#"{"name": 42}"#), // name should be string
        );

        let errors = validate_input_values(dir.path(), &inputs).errors;
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("development"));
        assert!(errors[0].contains("does not match schema"));
    }

    #[test]
    fn both_default_and_dev_invalid() {
        let (dir, inputs) = setup_template(
            SCHEMA,
            Some(r#"{"age": "not a number"}"#), // missing name, age wrong type
            Some(r#"[]"#),                      // wrong type entirely
        );

        let errors = validate_input_values(dir.path(), &inputs).errors;
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("default"));
        assert!(errors[1].contains("development"));
    }

    #[test]
    fn missing_default_file_reports_error() {
        let dir = tempdir().unwrap();

        let mut schema_file = File::create(dir.path().join("data.schema.json")).unwrap();
        write!(schema_file, "{SCHEMA}").unwrap();

        let inputs = vec![InputDefinition::Json(oicana::input::JsonInputDefinition {
            key: "data".to_string(),
            required: true,
            default: Some("nonexistent.json".to_string()),
            development: None,
            schema: Some("data.schema.json".to_string()),
            validate: true,
        })];

        let errors = validate_input_values(dir.path(), &inputs).errors;
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("failed to read default value file"));
    }

    #[test]
    fn invalid_json_in_file_reports_error() {
        let dir = tempdir().unwrap();

        let mut schema_file = File::create(dir.path().join("data.schema.json")).unwrap();
        write!(schema_file, "{SCHEMA}").unwrap();

        let mut f = File::create(dir.path().join("default.json")).unwrap();
        write!(f, "not valid json {{").unwrap();

        let inputs = vec![InputDefinition::Json(oicana::input::JsonInputDefinition {
            key: "data".to_string(),
            required: true,
            default: Some("default.json".to_string()),
            development: None,
            schema: Some("data.schema.json".to_string()),
            validate: true,
        })];

        let errors = validate_input_values(dir.path(), &inputs).errors;
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("not valid JSON"));
    }

    #[test]
    fn no_schema_skips_validation() {
        let dir = tempdir().unwrap();

        let mut f = File::create(dir.path().join("default.json")).unwrap();
        write!(f, "not even json").unwrap();

        let inputs = vec![InputDefinition::Json(oicana::input::JsonInputDefinition {
            key: "data".to_string(),
            required: true,
            default: Some("default.json".to_string()),
            development: None,
            schema: None,
            validate: true,
        })];

        let errors = validate_input_values(dir.path(), &inputs).errors;
        assert!(errors.is_empty(), "No schema means no validation");
    }

    #[test]
    fn missing_schema_file_reports_error() {
        let dir = tempdir().unwrap();

        let mut f = File::create(dir.path().join("default.json")).unwrap();
        write!(f, r#"{{"name": "Alice"}}"#).unwrap();

        let inputs = vec![InputDefinition::Json(oicana::input::JsonInputDefinition {
            key: "data".to_string(),
            required: true,
            default: Some("default.json".to_string()),
            development: None,
            schema: Some("missing.schema.json".to_string()),
            validate: true,
        })];

        let errors = validate_input_values(dir.path(), &inputs).errors;
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("failed to read schema file"));
    }

    fn json_input(default: Option<&str>, development: Option<&str>) -> InputDefinition {
        InputDefinition::Json(oicana::input::JsonInputDefinition {
            key: "data".to_string(),
            required: true,
            default: default.map(str::to_string),
            development: development.map(str::to_string),
            schema: None,
            validate: true,
        })
    }

    fn blob_input(default: Option<&str>, development: Option<&str>) -> InputDefinition {
        let fallback = |file: &str| FallbackBlobInput {
            file: file.to_string(),
            meta: None,
        };
        InputDefinition::Blob(BlobInputDefinition {
            key: "logo".to_string(),
            required: true,
            default: default.map(fallback),
            development: development.map(fallback),
        })
    }

    #[test]
    fn missing_dev_file_without_schema_reports_error() {
        let dir = tempdir().unwrap();
        let inputs = vec![json_input(None, Some("nope.json"))];

        let errors = validate_input_values(dir.path(), &inputs).errors;
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("failed to read development value file 'nope.json'"),
            "got: {errors:?}"
        );
    }

    #[test]
    fn missing_blob_fallback_files_report_errors() {
        let dir = tempdir().unwrap();
        let inputs = vec![blob_input(
            Some("missing-default.png"),
            Some("missing-dev.png"),
        )];

        let errors = validate_input_values(dir.path(), &inputs).errors;
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("failed to read default value file 'missing-default.png'"));
        assert!(errors[1].contains("failed to read development value file 'missing-dev.png'"));
    }

    #[test]
    fn existing_blob_fallback_file_passes() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("logo.png")).unwrap();
        let inputs = vec![blob_input(None, Some("logo.png"))];

        let issues = validate_input_values(dir.path(), &inputs);
        assert!(issues.errors.is_empty(), "got: {:?}", issues.errors);
        assert!(issues.warnings.is_empty(), "got: {:?}", issues.warnings);
    }

    #[test]
    fn required_input_without_any_fallback_warns() {
        let dir = tempdir().unwrap();
        let inputs = vec![json_input(None, None), blob_input(None, None)];

        let issues = validate_input_values(dir.path(), &inputs);
        assert!(issues.errors.is_empty(), "got: {:?}", issues.errors);
        assert_eq!(issues.warnings.len(), 2);
        assert!(issues.warnings[0].contains("Input 'data' is required"));
        assert!(issues.warnings[1].contains("Input 'logo' is required"));
    }

    #[test]
    fn optional_input_without_fallback_does_not_warn() {
        let dir = tempdir().unwrap();
        let InputDefinition::Json(mut json_def) = json_input(None, None) else {
            unreachable!("json_input builds a JSON input");
        };
        json_def.required = false;

        let issues = validate_input_values(dir.path(), &[InputDefinition::Json(json_def)]);
        assert!(issues.warnings.is_empty(), "got: {:?}", issues.warnings);
    }

    #[test]
    fn required_input_with_fallback_does_not_warn() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("dev.json")).unwrap();
        let inputs = vec![json_input(None, Some("dev.json"))];

        let issues = validate_input_values(dir.path(), &inputs);
        assert!(issues.warnings.is_empty(), "got: {:?}", issues.warnings);
    }
}
