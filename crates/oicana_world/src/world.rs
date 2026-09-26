use crate::diagnostics::{DiagnosticColor, TemplateDiagnostics};
use crate::fonts::{missing_font_families, FontCollection, FontSlot, FontSource};
use crate::{get_current_time, CompiledDocument, TemplateCompilationFailure};

use chrono::{DateTime, Datelike, Local};
use jsonschema::Validator;
use log::info;
use oicana_files::TemplateFiles;
use oicana_input::{ConflictingInput, InputDefinition, InputKind, TemplateInputs};
use oicana_template::manifest::ManifestValidationError;
use oicana_template::manifest::TemplateManifest;
use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;
use thiserror::Error;
use typst::diag::{FileError, FileResult, Warned};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, PathError, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

/// A world that provides access to fonts and template files.
pub struct OicanaWorld<Files: TemplateFiles> {
    main: FileId,
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<FontSlot>,
    now: OnceLock<DateTime<Local>>,
    manifest: TemplateManifest,
    validators: HashMap<String, Validator>,
    /// Whether to validate JSON inputs against their schemas before compilation.
    pub validate_inputs: bool,
    /// Color mode for diagnostic logs
    pub color: DiagnosticColor,
    /// Files access to the template.
    pub files: Files,
}

impl<Files: TemplateFiles> fmt::Debug for OicanaWorld<Files> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OicanaWorld")
            .field("main", &self.main)
            .field("manifest", &self.manifest)
            .field(
                "validators",
                &format!("{} schema(s)", self.validators.len()),
            )
            .field("validate_inputs", &self.validate_inputs)
            .field("color", &self.color)
            .finish_non_exhaustive()
    }
}

/// Build JSON schema validators from the template manifest and files.
///
/// Only builds validators for JSON inputs that have a schema and whose `validate`
/// flag is `true`.
fn build_validators<F: TemplateFiles>(
    manifest: &TemplateManifest,
    files: &F,
) -> Result<HashMap<String, Validator>, WorldCreationError> {
    let mut validators = HashMap::new();

    for input in &manifest.tool.oicana.inputs {
        let InputDefinition::Json(json_def) = input else {
            continue;
        };
        let Some(schema_path) = &json_def.schema else {
            continue;
        };
        if !json_def.validate {
            continue;
        }

        let vpath =
            VirtualPath::new(schema_path).map_err(|error| WorldCreationError::SchemaError {
                key: json_def.key.clone(),
                message: format!("Invalid schema path '{schema_path}': {error}"),
            })?;
        let file_id = FileId::new(RootedPath::new(VirtualRoot::Project, vpath));
        let schema_bytes = files
            .file(file_id)
            .map_err(|e| WorldCreationError::SchemaError {
                key: json_def.key.clone(),
                message: format!("Failed to read schema file '{schema_path}': {e}"),
            })?;

        let schema_value: serde_json::Value =
            serde_json::from_slice(&schema_bytes).map_err(|e| WorldCreationError::SchemaError {
                key: json_def.key.clone(),
                message: format!("Failed to parse schema file '{schema_path}': {e}"),
            })?;

        let validator = jsonschema::validator_for(&schema_value).map_err(|e| {
            WorldCreationError::SchemaError {
                key: json_def.key.clone(),
                message: format!("Failed to compile schema '{schema_path}': {e}"),
            }
        })?;

        info!(
            "Compiled JSON schema for input '{}' from '{}'",
            json_def.key, schema_path
        );
        validators.insert(json_def.key.clone(), validator);
    }

    Ok(validators)
}

/// Check the supplied inputs against the input definitions the manifest declares.
fn check_declarations(
    manifest: &TemplateManifest,
    inputs: &TemplateInputs,
) -> Result<(), InputMismatchError> {
    let definitions = &manifest.tool.oicana.inputs;
    let conflicting = inputs.conflicts().to_vec();
    let mut unknown = Vec::new();
    let mut wrong_kind = Vec::new();

    for (key, supplied) in inputs.kinds() {
        if conflicting.iter().any(|conflict| conflict.key == key) {
            continue;
        }
        match definitions.iter().find(|def| def.key() == key) {
            None => unknown.push(key.to_owned()),
            Some(definition) if definition.kind() != supplied => wrong_kind.push(WrongInputKind {
                key: key.to_owned(),
                declared: definition.kind(),
                supplied,
            }),
            Some(_) => {}
        }
    }

    if unknown.is_empty() && wrong_kind.is_empty() && conflicting.is_empty() {
        return Ok(());
    }

    Err(InputMismatchError {
        unknown,
        wrong_kind,
        conflicting,
        declared: definitions
            .iter()
            .map(|definition| definition.key().to_owned())
            .collect(),
    })
}

/// Validate JSON inputs against their schemas.
fn validate_against_schemas(
    validators: &HashMap<String, Validator>,
    inputs: &TemplateInputs,
) -> Result<(), InputValidationError> {
    for (key, validator) in validators {
        let Some(json_str) = inputs.get_str_value(key) else {
            continue;
        };

        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).map_err(|e| InputValidationError {
                key: key.clone(),
                errors: vec![format!("Invalid JSON: {e}")],
            })?;

        if !validator.is_valid(&parsed) {
            let errors: Vec<String> = validator
                .iter_errors(&parsed)
                .map(|error| {
                    let path = error.instance_path().to_string();
                    if path.is_empty() {
                        error.to_string()
                    } else {
                        format!("At {path}: {error}")
                    }
                })
                .collect();

            return Err(InputValidationError {
                key: key.clone(),
                errors,
            });
        }
    }
    Ok(())
}

impl<Files: TemplateFiles> OicanaWorld<Files> {
    /// Create a new Typst World.
    ///
    /// This will collect embedded fonts from Typst and fonts included in the template files.
    /// If JSON inputs have schemas defined in the manifest, the schemas are compiled at this
    /// point for fast validation during compilation.
    pub fn new(files: Files, manifest: TemplateManifest) -> Result<Self, WorldCreationError> {
        Self::new_with_fonts(files, manifest, &[])
    }

    /// Create a new Typst World with additional fonts provided by the host.
    pub fn new_with_fonts(
        files: Files,
        manifest: TemplateManifest,
        host_fonts: &[FontSource],
    ) -> Result<Self, WorldCreationError> {
        let main_path = VirtualPath::new(manifest.package.entrypoint.as_str())?;
        let main = FileId::new(RootedPath::new(VirtualRoot::Project, main_path));
        files.source(main)?;

        let validate_inputs = manifest.tool.oicana.validate_json_inputs_by_default;
        let validators = build_validators(&manifest, &files)?;

        let library = Library::builder()
            .with_inputs(TemplateInputs::new().to_dict())
            .build();

        let mut searcher = FontCollection::new();
        searcher.collect(&files, host_fonts);

        let missing = missing_font_families(&searcher.book, manifest.required_font_families());
        if !missing.is_empty() {
            return Err(WorldCreationError::MissingFonts(missing));
        }

        Ok(Self {
            main,
            library: LazyHash::new(library),
            book: LazyHash::new(searcher.book),
            fonts: searcher.fonts,
            now: OnceLock::new(),
            manifest,
            validators,
            validate_inputs,
            color: DiagnosticColor::Ansi,
            files,
        })
    }

    /// Update the inputs of the World.
    ///
    /// Rejects inputs the manifest does not declare and
    /// validates JSON inputs against their schemas.
    pub fn update_inputs(&mut self, inputs: TemplateInputs) -> Result<(), InputError> {
        check_declarations(&self.manifest, &inputs)?;
        if self.validate_inputs {
            validate_against_schemas(&self.validators, &inputs)?;
        }
        self.library = LazyHash::new(Library::builder().with_inputs(inputs.to_dict()).build());
        self.reset_time();
        Ok(())
    }

    /// Compile the template world.
    ///
    /// Note: This does not automatically evict the comemo cache. Use [`evict_cache`] to
    /// manually manage cache eviction if needed.
    pub fn compile(&mut self) -> Result<CompiledDocument, TemplateCompilationFailure> {
        let start = get_current_time();
        let Warned { output, warnings } = typst::compile(self);
        info!("Compiled Document in {}ms", get_current_time() - start);
        let warnings = if warnings.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&self.format_diagnostics(warnings)).into())
        };

        match output {
            Ok(document) => Ok(CompiledDocument { document, warnings }),
            Err(diagnostics) => Err(TemplateCompilationFailure {
                error: String::from_utf8_lossy(&self.format_diagnostics(diagnostics)).into(),
                warnings,
            }),
        }
    }

    /// Manifest of the Oicana template
    pub fn manifest(&self) -> &TemplateManifest {
        &self.manifest
    }

    /// Reset the cached timestamp so the next compilation gets a fresh value.
    pub fn reset_time(&mut self) {
        self.now = OnceLock::new();
    }
}

/// An error that occurs during world construction.
#[derive(Error, Debug)]
pub enum WorldCreationError {
    /// Error while accessing a file in the template
    #[error("Failed to access a required file: {0}")]
    FileError(FileError),
    /// Error in the template manifest
    #[error("There was an issue with the package manifest: {0}")]
    ManifestError(ManifestValidationError),
    /// A JSON schema could not be loaded or compiled
    #[error("Schema error for input '{key}': {message}")]
    SchemaError {
        /// The key of the input whose schema failed
        key: String,
        /// Description of what went wrong
        message: String,
    },
    /// The entrypoint configured in the manifest is not a valid path
    #[error("The entrypoint configured in the manifest is not a valid path: {0}")]
    InvalidEntrypoint(#[from] PathError),
    /// The template requires font families that are not available
    #[error(
        "The template requires font families that are not available: {}. \
         Either add the fonts to the template or register them with the host.",
        .0.join(", ")
    )]
    MissingFonts(Vec<String>),
}

impl From<FileError> for WorldCreationError {
    fn from(error: FileError) -> Self {
        WorldCreationError::FileError(error)
    }
}

impl From<ManifestValidationError> for WorldCreationError {
    fn from(error: ManifestValidationError) -> Self {
        WorldCreationError::ManifestError(error)
    }
}

/// A supplied input whose kind does not match the kind its definition declares
#[derive(Debug, PartialEq, Eq)]
pub struct WrongInputKind {
    /// The key of the mismatched input
    pub key: String,
    /// The kind the manifest declares for this key
    pub declared: InputKind,
    /// The kind that was supplied
    pub supplied: InputKind,
}

/// The supplied inputs do not match the template's input definitions
#[derive(Debug)]
pub struct InputMismatchError {
    /// Supplied keys that no input definition declares
    pub unknown: Vec<String>,
    /// Supplied inputs whose kind does not match their declaration
    pub wrong_kind: Vec<WrongInputKind>,
    /// Keys supplied as more than one kind of input
    pub conflicting: Vec<ConflictingInput>,
    /// The keys the manifest declares
    pub declared: Vec<String>,
}

impl fmt::Display for InputMismatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut problems = Vec::new();
        for key in &self.unknown {
            problems.push(format!("The template declares no input for '{key}'."));
        }
        for wrong in &self.wrong_kind {
            problems.push(format!(
                "Input '{}' is declared as a {} input, but a {} value was supplied.",
                wrong.key, wrong.declared, wrong.supplied
            ));
        }
        for conflict in &self.conflicting {
            problems.push(format!(
                "Input '{}' was supplied as both a {} and a {} input.",
                conflict.key, conflict.first, conflict.second
            ));
        }

        write!(f, "{}", problems.join(" "))?;
        if !self.unknown.is_empty() {
            write!(
                f,
                " Declared inputs: {}",
                if self.declared.is_empty() {
                    "none".to_owned()
                } else {
                    quoted_list(&self.declared)
                }
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for InputMismatchError {}

fn quoted_list(keys: &[String]) -> String {
    keys.iter()
        .map(|key| format!("'{key}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// An input could not be applied to a template
#[derive(Debug, Error)]
pub enum InputError {
    /// The supplied inputs do not match the template's input definitions
    #[error(transparent)]
    Mismatch(#[from] InputMismatchError),
    /// A JSON input did not match its schema
    #[error(transparent)]
    Validation(#[from] InputValidationError),
}

/// A JSON input did not match its schema
#[derive(Debug)]
pub struct InputValidationError {
    /// The key of the input that failed validation
    pub key: String,
    /// The validation errors
    pub errors: Vec<String>,
}

impl fmt::Display for InputValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Input '{}' failed schema validation:\n{}",
            self.key,
            self.errors
                .iter()
                .map(|e| format!("  - {e}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

impl std::error::Error for InputValidationError {}

impl<Files: TemplateFiles> World for OicanaWorld<Files> {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.files.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts[index].get()
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        let now = self.now.get_or_init(Local::now);

        let naive = match offset {
            None => now.naive_local(),
            Some(offset) => {
                let seconds = offset.seconds().trunc();
                if !seconds.is_finite()
                    || seconds < f64::from(i32::MIN)
                    || seconds > f64::from(i32::MAX)
                {
                    return None;
                }
                now.naive_utc()
                    .checked_add_signed(chrono::Duration::seconds(seconds as i64))?
            }
        };

        Datetime::from_ymd(
            naive.year(),
            naive.month().try_into().ok()?,
            naive.day().try_into().ok()?,
        )
    }
}

/// Evict cached compilation artifacts from the global comemo cache.
///
/// The comemo cache is global and shared across all `OicanaWorld` instances.
/// This function removes memoized results whose age is larger than or equal to `max_age`.
///
/// # How Cache Aging Works
///
/// - Each cache entry has an age counter
/// - Age increases by 1 during each eviction
/// - Age resets to 0 when the entry produces a cache hit (is used)
/// - Entries with age >= `max_age` are removed
///
/// # Parameters
///
/// * `max_age` - Maximum age threshold:
///   - `0` - Removes all cache entries (full clear)
///   - `n` - Keeps entries used within the last n eviction cycles
///
/// # Example
///
/// ```rust
/// use oicana_world::evict_cache;
///
/// // Clear all cache entries
/// evict_cache(0);
///
/// // Keep entries used in last 30 eviction cycles
/// evict_cache(30);
/// ```
pub fn evict_cache(max_age: usize) {
    comemo::evict(max_age);
}

#[cfg(test)]
mod tests {
    use crate::manifest::{OicanaWorldFiles, OicanaWorldManifestError};
    use crate::world::{
        InputError, InputValidationError, OicanaWorld, WorldCreationError, WrongInputKind,
    };
    use oicana_files::preloaded::PreloadedTemplate;
    use oicana_input::{InputKind, TemplateInputs};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use typst::diag::FileError;
    use typst::foundations::Duration;

    /// Unwrap the schema-validation variant of an [`InputError`].
    fn expect_validation(error: InputError) -> InputValidationError {
        match error {
            InputError::Validation(error) => error,
            other => panic!("expected a schema validation error, got {other:?}"),
        }
    }

    #[test]
    fn can_build_world_with_minimal_template() {
        let mut files = HashMap::new();
        files.insert("main.typ".to_owned(), "Test".to_owned());
        files.insert(
            "typst.toml".to_owned(),
            r#"
        [package]
        entrypoint = "main.typ"
        name = "test"
        version = "0.1.0"

        [tool.oicana]
        manifest_version = 1
        "#
            .to_owned(),
        );
        let files = PreloadedTemplate::new(files);
        let manifest = files.manifest().expect("should be able to parse manifest");

        OicanaWorld::new(files, manifest).expect("Failed to create world");
    }

    #[test]
    fn refuses_to_read_a_manifest_from_a_newer_oicana() {
        let mut files = HashMap::new();
        files.insert("main.typ".to_owned(), "Test".to_owned());
        files.insert(
            "typst.toml".to_owned(),
            r#"
        [package]
        entrypoint = "main.typ"
        name = "test"
        version = "0.1.0"

        [tool.oicana]
        manifest_version = 99
        "#
            .to_owned(),
        );
        let files = PreloadedTemplate::new(files);

        let Err(OicanaWorldManifestError::UnsupportedManifestVersion(error)) = files.manifest()
        else {
            panic!("Read a manifest declaring an unsupported version")
        };

        assert!(
            error
                .to_string()
                .contains("Unsupported manifest_version 99"),
            "got: {error}"
        );
    }

    #[test]
    fn fails_to_build_world_without_typst_toml_file() {
        let mut files = HashMap::new();
        files.insert("some_wrong_file_name.typ".to_owned(), "Test".to_owned());
        let files = PreloadedTemplate::new(files);

        let Err(OicanaWorldManifestError::NoManifest(file_error)) = files.manifest() else {
            panic!("Created a world without main template file or with wrong error")
        };

        assert_eq!(
            file_error,
            FileError::NotFound(PathBuf::from("/typst.toml"))
        )
    }

    #[test]
    fn fails_to_parse_invalid_package_meta() {
        let mut files = HashMap::new();
        files.insert(
            "typst.toml".to_owned(),
            r#"
        [package]
        entrypoint = "not_main.typ"

        [tool.oicana]
        manifest_version = 1
        "#
            .to_owned(),
        );
        files.insert("main.typ".to_owned(), "Test".to_owned());
        let files = PreloadedTemplate::new(files);

        let Err(OicanaWorldManifestError::InvalidManifest(file_error)) = files.manifest() else {
            panic!("Created a world with invalid package meta or got unexpected error")
        };

        assert_eq!(file_error.message(), "missing field `name`")
    }

    #[test]
    fn fails_to_build_world_without_entry_file() {
        let mut files = HashMap::new();
        files.insert(
            "typst.toml".to_owned(),
            r#"
        [package]
        entrypoint = "not_main.typ"
        name = "test"
        version = "0.1.0"

        [tool.oicana]
        manifest_version = 1
        "#
            .to_owned(),
        );
        files.insert("main.typ".to_owned(), "Test".to_owned());
        let files = PreloadedTemplate::new(files);
        let manifest = files.manifest().expect("should be able to parse manifest");

        let Err(error) = OicanaWorld::new(files, manifest) else {
            panic!("Created a world without main template file")
        };
        assert!(error.to_string().contains("not_main.typ"), "got: {error}");
        let WorldCreationError::FileError(file_error) = error else {
            panic!("Wrong error: {error}")
        };

        assert_eq!(
            file_error,
            FileError::NotFound(PathBuf::from("/not_main.typ"))
        )
    }

    fn simple_manifest() -> &'static str {
        r#"
        [package]
        name = "test"
        version = "0.1.0"
        entrypoint = "main.typ"

        [tool.oicana]
        manifest_version = 1
        "#
    }

    fn template_with(manifest: &str, main_typ: &str) -> PreloadedTemplate {
        let mut files = HashMap::new();
        files.insert("typst.toml".to_owned(), manifest.to_owned());
        files.insert("main.typ".to_owned(), main_typ.to_owned());
        PreloadedTemplate::new(files)
    }

    #[test]
    fn compiles_simple_template() {
        let files = template_with(
            simple_manifest(),
            "#set page(width: 200pt, height: 100pt)\nHello, World!",
        );
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let result = world.compile();

        assert!(result.is_ok());
        let compiled = result.unwrap();
        assert!(compiled.warnings.is_none());
        assert_eq!(compiled.document.pages().len(), 1);
    }

    #[test]
    fn compiles_multipage_template() {
        let files = template_with(simple_manifest(), "#set page(width: 200pt, height: 100pt)\nPage 1\n#pagebreak()\nPage 2\n#pagebreak()\nPage 3");
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let compiled = world.compile().unwrap();

        assert_eq!(compiled.document.pages().len(), 3);
    }

    #[test]
    fn fails_to_compile_invalid_template() {
        let files = template_with(simple_manifest(), "#invalid_typst_syntax #(");
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let result = world.compile();

        assert!(result.is_err());
    }

    #[test]
    fn compiles_empty_template() {
        let files = template_with(simple_manifest(), "");
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let compiled = world.compile().unwrap();

        assert_eq!(compiled.document.pages().len(), 1);
    }

    #[test]
    fn compilation_with_warnings_includes_warning_message() {
        let files = template_with(
            simple_manifest(),
            "#set text(font: \"NonexistentFont\")\nContent",
        );
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let compiled = world.compile().unwrap();

        assert!(compiled.warnings.is_some());
    }

    #[test]
    fn manifest_returns_correct_template_manifest() {
        let files = template_with(
            r#"
            [package]
            name = "test-template"
            version = "0.1.0"
            entrypoint = "main.typ"

            [tool.oicana]
            manifest_version = 1
            "#,
            "Test",
        );
        let manifest = files.manifest().unwrap();
        let world = OicanaWorld::new(files, manifest).unwrap();

        let returned_manifest = world.manifest();

        assert_eq!(returned_manifest.package.name, "test-template");
        assert_eq!(returned_manifest.package.version.to_string(), "0.1.0");
    }

    fn schema_manifest() -> &'static str {
        r#"
        [package]
        name = "test"
        version = "0.1.0"
        entrypoint = "main.typ"

        [tool.oicana]
        manifest_version = 1

        [[tool.oicana.inputs]]
        type = "json"
        key = "data"
        schema = "data.schema.json"
        "#
    }

    fn simple_schema() -> &'static str {
        r#"{
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer", "minimum": 0 }
            },
            "required": ["name"]
        }"#
    }

    fn template_with_schema(manifest: &str, main_typ: &str, schema: &str) -> PreloadedTemplate {
        let mut files = HashMap::new();
        files.insert("typst.toml".to_owned(), manifest.to_owned());
        files.insert("main.typ".to_owned(), main_typ.to_owned());
        files.insert("data.schema.json".to_owned(), schema.to_owned());
        PreloadedTemplate::new(files)
    }

    #[test]
    fn rejects_an_input_the_manifest_does_not_declare() {
        use oicana_input::JsonInput;

        let files = template_with_schema(schema_manifest(), "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("dtaa", r#"{"name": "Alice"}"#));

        let error = world.update_inputs(inputs).unwrap_err();
        let InputError::Mismatch(error) = error else {
            panic!("expected a mismatch, got {error:?}");
        };
        assert_eq!(error.unknown, vec!["dtaa".to_owned()]);
        assert_eq!(error.declared, vec!["data".to_owned()]);
        assert_eq!(
            error.to_string(),
            "The template declares no input for 'dtaa'. Declared inputs: 'data'"
        );
    }

    #[test]
    fn rejects_a_blob_value_for_a_key_declared_as_json() {
        use oicana_input::BlobInput;
        use typst::foundations::Bytes;

        let files = template_with_schema(schema_manifest(), "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let mut inputs = TemplateInputs::new();
        inputs.with_input(BlobInput::new("data", Bytes::new([1u8, 2].as_slice())));

        let error = world.update_inputs(inputs).unwrap_err();
        let InputError::Mismatch(error) = error else {
            panic!("expected a mismatch, got {error:?}");
        };
        assert!(error.unknown.is_empty());
        assert_eq!(
            error.wrong_kind,
            vec![WrongInputKind {
                key: "data".to_owned(),
                declared: InputKind::Json,
                supplied: InputKind::Blob,
            }]
        );
        assert_eq!(
            error.to_string(),
            "Input 'data' is declared as a json input, but a blob value was supplied."
        );
    }

    #[test]
    fn rejects_a_json_value_for_a_key_declared_as_blob() {
        use oicana_input::JsonInput;

        let blob_manifest = r#"
        [package]
        name = "test"
        version = "0.1.0"
        entrypoint = "main.typ"

        [tool.oicana]
        manifest_version = 1

        [[tool.oicana.inputs]]
        type = "blob"
        key = "logo"
        "#;
        let files = template_with(blob_manifest, "Test");
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("logo", "{}"));

        assert_eq!(
            world.update_inputs(inputs).unwrap_err().to_string(),
            "Input 'logo' is declared as a blob input, but a json value was supplied."
        );
    }

    #[test]
    fn rejects_a_key_supplied_as_two_kinds() {
        use oicana_input::BlobInput;
        use oicana_input::JsonInput;
        use typst::foundations::Bytes;

        let files = template_with_schema(schema_manifest(), "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("data", r#"{"name": "Alice"}"#));
        inputs.with_input(BlobInput::new("data", Bytes::new([1u8].as_slice())));

        assert_eq!(
            world.update_inputs(inputs).unwrap_err().to_string(),
            "Input 'data' was supplied as both a json and a blob input."
        );
    }

    #[test]
    fn reports_every_input_problem_at_once() {
        use oicana_input::BlobInput;
        use oicana_input::JsonInput;
        use typst::foundations::Bytes;

        let files = template_with_schema(schema_manifest(), "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let mut inputs = TemplateInputs::new();
        inputs.with_input(BlobInput::new("data", Bytes::new([1u8].as_slice())));
        inputs.with_input(JsonInput::new("dtaa", "{}"));

        let InputError::Mismatch(error) = world.update_inputs(inputs).unwrap_err() else {
            panic!("expected a mismatch");
        };
        assert_eq!(error.unknown, vec!["dtaa".to_owned()]);
        assert_eq!(error.wrong_kind.len(), 1);
    }

    #[test]
    fn rejects_undeclared_inputs_even_with_schema_validation_disabled() {
        use oicana_input::JsonInput;

        let files = template_with_schema(schema_manifest(), "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();
        world.validate_inputs = false;

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("dtaa", r#"{"name": "Alice"}"#));

        assert!(matches!(
            world.update_inputs(inputs),
            Err(InputError::Mismatch(_))
        ));
    }

    #[test]
    fn builds_world_with_schema() {
        let files = template_with_schema(schema_manifest(), "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let world = OicanaWorld::new(files, manifest).unwrap();

        assert_eq!(world.validators.len(), 1);
        assert!(world.validators.contains_key("data"));
    }

    #[test]
    fn validates_valid_json_input() {
        use oicana_input::JsonInput;

        let files = template_with_schema(schema_manifest(), "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("data", r#"{"name": "Alice", "age": 30}"#));

        let result = world.update_inputs(inputs);
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_json_missing_required_field() {
        use oicana_input::JsonInput;

        let files = template_with_schema(schema_manifest(), "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("data", r#"{"age": 30}"#));

        let result = world.update_inputs(inputs);
        assert!(result.is_err());
        let err = expect_validation(result.unwrap_err());
        assert_eq!(err.key, "data");
        assert!(!err.errors.is_empty());
        assert!(
            err.errors[0].contains("name"),
            "Error should mention the missing 'name' field: {}",
            err.errors[0]
        );
    }

    #[test]
    fn rejects_json_with_wrong_type() {
        use oicana_input::JsonInput;

        let files = template_with_schema(schema_manifest(), "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("data", r#"{"name": 123}"#));

        let result = world.update_inputs(inputs);
        assert!(result.is_err());
        let err = expect_validation(result.unwrap_err());
        assert_eq!(err.key, "data");
    }

    #[test]
    fn rejects_invalid_json_syntax() {
        use oicana_input::JsonInput;

        let files = template_with_schema(schema_manifest(), "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("data", r#"{not valid json}"#));

        let result = world.update_inputs(inputs);
        assert!(result.is_err());
        let err = expect_validation(result.unwrap_err());
        assert!(err.errors[0].contains("Invalid JSON"));
    }

    #[test]
    fn skips_validation_for_inputs_without_schema() {
        use oicana_input::JsonInput;

        let manifest_with_unschemad_input = r#"
        [package]
        name = "test"
        version = "0.1.0"
        entrypoint = "main.typ"

        [tool.oicana]
        manifest_version = 1

        [[tool.oicana.inputs]]
        type = "json"
        key = "data"
        schema = "data.schema.json"

        [[tool.oicana.inputs]]
        type = "json"
        key = "other"
        "#;
        let files = template_with_schema(manifest_with_unschemad_input, "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        // "other" is declared without a schema, so any value should be accepted
        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("other", r#"literally anything"#));

        let result = world.update_inputs(inputs);
        assert!(result.is_ok());
    }

    #[test]
    fn no_validators_for_template_without_schemas() {
        let files = template_with(simple_manifest(), "Test");
        let manifest = files.manifest().unwrap();
        let world = OicanaWorld::new(files, manifest).unwrap();

        assert!(world.validators.is_empty());
    }

    #[test]
    fn fails_to_create_world_with_missing_schema_file() {
        let mut files = HashMap::new();
        files.insert("typst.toml".to_owned(), schema_manifest().to_owned());
        files.insert("main.typ".to_owned(), "Test".to_owned());
        // Deliberately NOT inserting data.schema.json
        let files = PreloadedTemplate::new(files);
        let manifest = files.manifest().unwrap();

        let result = OicanaWorld::new(files, manifest);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, WorldCreationError::SchemaError { .. }),
            "Expected SchemaError, got: {err:?}"
        );
    }

    #[test]
    fn fails_to_create_world_with_invalid_schema_json() {
        let files = template_with_schema(schema_manifest(), "Test", "not valid json {{{");
        let manifest = files.manifest().unwrap();

        let result = OicanaWorld::new(files, manifest);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, WorldCreationError::SchemaError { .. }),
            "Expected SchemaError, got: {err:?}"
        );
    }

    #[test]
    fn accepts_invalid_json_when_validation_disabled() {
        use oicana_input::JsonInput;

        let files = template_with_schema(schema_manifest(), "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();
        world.validate_inputs = false;

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("data", r#"{"age": 30}"#)); // missing required "name"

        let result = world.update_inputs(inputs);
        assert!(result.is_ok());
    }

    #[test]
    fn per_input_validate_false_skips_validator_creation() {
        let manifest = r#"
        [package]
        name = "test"
        version = "0.1.0"
        entrypoint = "main.typ"

        [tool.oicana]
        manifest_version = 1

        [[tool.oicana.inputs]]
        type = "json"
        key = "data"
        schema = "data.schema.json"
        validate = false
        "#;
        let files = template_with_schema(manifest, "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let world = OicanaWorld::new(files, manifest).unwrap();

        assert!(world.validators.is_empty());
    }

    #[test]
    fn per_input_validate_false_accepts_invalid_json() {
        use oicana_input::JsonInput;

        let manifest = r#"
        [package]
        name = "test"
        version = "0.1.0"
        entrypoint = "main.typ"

        [tool.oicana]
        manifest_version = 1

        [[tool.oicana.inputs]]
        type = "json"
        key = "data"
        schema = "data.schema.json"
        validate = false
        "#;
        let files = template_with_schema(manifest, "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("data", r#"{"age": 30}"#));
        assert!(world.update_inputs(inputs).is_ok());
    }

    #[test]
    fn per_input_validate_true_is_default_and_validates() {
        use oicana_input::JsonInput;

        let files = template_with_schema(schema_manifest(), "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        assert_eq!(world.validators.len(), 1);

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("data", r#"{"age": 30}"#));
        assert!(world.update_inputs(inputs).is_err());
    }

    #[test]
    fn mixed_validate_flags_only_validates_enabled_inputs() {
        use oicana_input::JsonInput;

        let manifest = r#"
        [package]
        name = "test"
        version = "0.1.0"
        entrypoint = "main.typ"

        [tool.oicana]
        manifest_version = 1

        [[tool.oicana.inputs]]
        type = "json"
        key = "validated"
        schema = "data.schema.json"
        validate = true

        [[tool.oicana.inputs]]
        type = "json"
        key = "unvalidated"
        schema = "data.schema.json"
        validate = false
        "#;
        let files = template_with_schema(manifest, "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        assert_eq!(world.validators.len(), 1);
        assert!(world.validators.contains_key("validated"));
        assert!(!world.validators.contains_key("unvalidated"));

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("unvalidated", r#"{"age": 30}"#));
        assert!(world.update_inputs(inputs).is_ok());

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("validated", r#"{"age": 30}"#));
        assert!(world.update_inputs(inputs).is_err());
    }

    #[test]
    fn manifest_validate_json_inputs_by_default_false_disables_validation_by_default() {
        use oicana_input::JsonInput;

        let manifest = r#"
        [package]
        name = "test"
        version = "0.1.0"
        entrypoint = "main.typ"

        [tool.oicana]
        manifest_version = 1
        validate_json_inputs_by_default = false

        [[tool.oicana.inputs]]
        type = "json"
        key = "data"
        schema = "data.schema.json"
        "#;
        let files = template_with_schema(manifest, "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        assert_eq!(world.validators.len(), 1);
        assert!(!world.validate_inputs);

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("data", r#"{"age": 30}"#));
        assert!(world.update_inputs(inputs).is_ok());
    }

    #[test]
    fn manifest_validate_json_inputs_by_default_true_is_default() {
        let files = template_with_schema(schema_manifest(), "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let world = OicanaWorld::new(files, manifest).unwrap();

        assert!(world.validate_inputs);
    }

    #[test]
    fn manifest_validate_json_inputs_by_default_false_can_be_overridden_at_runtime() {
        use oicana_input::JsonInput;

        let manifest = r#"
        [package]
        name = "test"
        version = "0.1.0"
        entrypoint = "main.typ"

        [tool.oicana]
        manifest_version = 1
        validate_json_inputs_by_default = false

        [[tool.oicana.inputs]]
        type = "json"
        key = "data"
        schema = "data.schema.json"
        "#;
        let files = template_with_schema(manifest, "Test", simple_schema());
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        world.validate_inputs = true;

        let mut inputs = TemplateInputs::new();
        inputs.with_input(JsonInput::new("data", r#"{"age": 30}"#));
        assert!(world.update_inputs(inputs).is_err());
    }

    #[test]
    fn validation_error_display_is_readable() {
        use super::InputValidationError;

        let err = InputValidationError {
            key: "invoice".to_string(),
            errors: vec![
                "\"name\" is a required property".to_string(),
                "At /age: -5 is less than the minimum of 0".to_string(),
            ],
        };

        let display = err.to_string();
        assert!(display.contains("invoice"));
        assert!(display.contains("name"));
        assert!(display.contains("/age"));
    }

    #[test]
    fn today_with_extreme_offset_returns_none() {
        use typst::World;

        let files = template_with(simple_manifest(), "Test");
        let manifest = files.manifest().unwrap();
        let world = OicanaWorld::new(files, manifest).unwrap();

        let seconds = |seconds| Duration::construct(seconds, 0, 0, 0, 0);

        // An offset large enough to push the date out of range must not panic.
        assert!(world.today(Some(seconds(100_000_000_000))).is_none());
        assert!(world.today(Some(seconds(-100_000_000_000))).is_none());
        assert!(world.today(Some(seconds(i64::MAX))).is_none());
        assert!(world.today(Some(seconds(i64::MIN))).is_none());

        assert!(world.today(Some(seconds(0))).is_some());
        assert!(world.today(None).is_some());
    }

    #[test]
    fn update_inputs_refreshes_the_cached_timestamp() {
        use chrono::TimeZone;
        use typst::foundations::Datetime;
        use typst::World;

        let files = template_with(simple_manifest(), "Test");
        let manifest = files.manifest().unwrap();
        let mut world = OicanaWorld::new(files, manifest).unwrap();

        let stale = chrono::Local.with_ymd_and_hms(2000, 1, 2, 3, 4, 5).unwrap();
        world.now.set(stale).unwrap();
        assert_eq!(world.today(None), Datetime::from_ymd(2000, 1, 2));

        world
            .update_inputs(TemplateInputs::new())
            .expect("inputs should be accepted");

        assert_ne!(world.today(None), Datetime::from_ymd(2000, 1, 2));
    }
}
