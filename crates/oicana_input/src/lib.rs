//! Definitions for Oicana inputs.

use log::warn;
use serde::{Deserialize, Serialize};
use std::fmt;
use typst::foundations::{Dict, Str, Value};

mod input;
mod input_definition;

pub use input::{Blob, BlobInput, ImageFormat, JsonInput};
pub use input_definition::{
    BlobInputDefinition, FallbackBlobInput, InputDefinition, JsonInputDefinition,
};

/// The kind of an input.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum InputKind {
    /// A JSON input.
    Json,
    /// A blob input.
    Blob,
}

impl fmt::Display for InputKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputKind::Json => write!(f, "json"),
            InputKind::Blob => write!(f, "blob"),
        }
    }
}

/// An input value.
pub trait Input {
    /// The key of the input.
    ///
    /// This is the identifier of the input definition this input value belongs to.
    fn key(&self) -> Str;

    /// The kind of this input.
    fn kind(&self) -> InputKind;

    /// Create a Typst value to be passed into the template.
    fn to_value(self) -> Value;
}

/// The same key was supplied as two different kinds of input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictingInput {
    /// The key that was supplied twice.
    pub key: String,
    /// The kind supplied first.
    pub first: InputKind,
    /// The kind supplied second.
    pub second: InputKind,
}

/// Combine template inputs.
#[derive(Debug, Clone)]
pub struct TemplateInputs {
    inputs: Dict,
    kinds: Vec<(Str, InputKind)>,
    conflicts: Vec<ConflictingInput>,
    config: CompilationConfig,
}

impl Default for TemplateInputs {
    fn default() -> Self {
        TemplateInputs::new()
    }
}

impl TemplateInputs {
    /// Create a new and empty inputs collection.
    pub fn new() -> Self {
        TemplateInputs {
            inputs: Dict::new(),
            kinds: Vec::new(),
            conflicts: Vec::new(),
            config: CompilationConfig::production(),
        }
    }

    /// Add a compilation configuration to the template inputs.
    pub fn with_config(&mut self, config: CompilationConfig) -> &mut Self {
        self.config = config;
        self
    }

    /// Add an input to the collection.
    pub fn with_input<I: Input>(&mut self, input: I) -> &mut Self {
        let key = input.key();
        let kind = input.kind();
        match self.kinds.iter().find(|(existing, _)| *existing == key) {
            Some((_, previous)) if *previous != kind => {
                self.conflicts.push(ConflictingInput {
                    key: key.to_string(),
                    first: *previous,
                    second: kind,
                });
            }
            Some(_) => warn!("An input is overwriting a previous input value!"),
            None => self.kinds.push((key.clone(), kind)),
        }
        self.inputs.insert(key, input.to_value());
        self
    }

    /// Check if a value has been supplied for the given key.
    pub fn contains(&self, key: &str) -> bool {
        self.inputs.contains(&Str::from(key))
    }

    /// The key and kind of every supplied input, in the order they were added.
    pub fn kinds(&self) -> impl Iterator<Item = (&str, InputKind)> {
        self.kinds.iter().map(|(key, kind)| (key.as_str(), *kind))
    }

    /// Keys that were supplied as more than one kind of input.
    pub fn conflicts(&self) -> &[ConflictingInput] {
        &self.conflicts
    }

    /// Get the string value of an input by key, if it exists.
    pub fn get_str_value(&self, key: &str) -> Option<String> {
        match self.inputs.at(Str::from(key), None) {
            Ok(Value::Str(s)) => Some(s.to_string()),
            _ => None,
        }
    }

    /// Build the Typst [`Dict`] that contains all previously added inputs and configuration.
    pub fn to_dict(self) -> Dict {
        let mut combined_inputs = Dict::new();
        combined_inputs.insert("oicana-inputs".into(), Value::Dict(self.inputs));
        combined_inputs.insert("oicana-config".into(), Value::Dict(self.config.into()));

        combined_inputs
    }
}

/// Configuration for template compilation
///
/// These values are passed into the template
#[derive(Debug, Clone, Copy)]
pub struct CompilationConfig {
    mode: CompilationMode,
}

/// Modes of compilation
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum CompilationMode {
    /// Compile the template in production mode, ignoring development values for inputs.
    #[serde(alias = "Production", alias = "PRODUCTION", alias = "prod")]
    Production,
    /// Compile the template in development mode using development values of inputs if configured.
    #[serde(alias = "Development", alias = "DEVELOPMENT", alias = "dev")]
    Development,
}

impl CompilationMode {
    fn is_production(&self) -> bool {
        matches!(self, CompilationMode::Production)
    }
}

impl CompilationConfig {
    /// Create a new configuration
    pub fn new(mode: CompilationMode) -> Self {
        CompilationConfig { mode }
    }

    /// Configuration for a production template compilation
    ///
    /// This will prevent the template from using development fallback values.
    pub fn production() -> Self {
        CompilationConfig {
            mode: CompilationMode::Production,
        }
    }

    /// Configuration for a development template compilation
    ///
    /// This will allow the template to use development fallback values.
    pub fn development() -> Self {
        CompilationConfig {
            mode: CompilationMode::Development,
        }
    }
}

impl From<CompilationConfig> for Dict {
    fn from(value: CompilationConfig) -> Self {
        let mut dict = Dict::new();
        dict.insert("production".into(), Value::Bool(value.mode.is_production()));

        dict
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlobInput, JsonInput};
    use typst::foundations::Bytes;

    #[test]
    fn combines_blob_and_string_inputs() {
        let mut inputs = TemplateInputs::new();
        let json_input = "{\"foo\": \"bar\"}".to_string();
        inputs
            .with_input(JsonInput::new("data", json_input))
            .with_input(BlobInput::new("blob1", Bytes::new([1u8, 2, 3].as_slice())))
            .with_input(BlobInput::new("blob2", Bytes::new([4u8].as_slice())));

        let Value::Dict(inputs) = inputs
            .to_dict()
            .at("oicana-inputs".into(), None)
            .expect("No inputs built!")
        else {
            panic!("Inputs should be dictionary!")
        };

        assert_eq!(inputs.len(), 3);
        assert!(inputs.contains("data"));
        assert!(inputs.contains("blob1"));
        assert!(inputs.contains("blob2"));
    }

    #[test]
    fn a_same_kind_duplicate_overwrites_without_conflict() {
        let mut inputs = TemplateInputs::new();
        inputs
            .with_input(JsonInput::new("data", "1"))
            .with_input(JsonInput::new("data", "2"));

        assert!(
            inputs.conflicts().is_empty(),
            "re-supplying the same kind is a deliberate override"
        );
        assert_eq!(inputs.get_str_value("data").as_deref(), Some("2"));
    }

    #[test]
    fn a_cross_kind_duplicate_is_recorded_as_a_conflict() {
        let mut inputs = TemplateInputs::new();
        inputs
            .with_input(JsonInput::new("data", "1"))
            .with_input(BlobInput::new("data", Bytes::new([1u8].as_slice())));

        assert_eq!(
            inputs.conflicts(),
            [ConflictingInput {
                key: "data".to_owned(),
                first: InputKind::Json,
                second: InputKind::Blob,
            }]
        );
    }

    #[test]
    fn kinds_are_reported_in_insertion_order() {
        let mut inputs = TemplateInputs::new();
        inputs
            .with_input(JsonInput::new("b", "1"))
            .with_input(BlobInput::new("a", Bytes::new([1u8].as_slice())));

        assert_eq!(
            inputs.kinds().collect::<Vec<_>>(),
            vec![("b", InputKind::Json), ("a", InputKind::Blob)]
        );
    }

    #[test]
    fn inputs_with_same_key_overwrite_old_value() {
        let mut inputs = TemplateInputs::new();
        let json_input = "{\"foo\": \"bar\"}".to_string();
        inputs
            .with_input(JsonInput::new("data", json_input))
            .with_input(BlobInput::new("blob1", Bytes::new([1u8, 2, 3].as_slice())))
            .with_input(BlobInput::new("data", Bytes::new([4u8].as_slice())));

        let Value::Dict(inputs) = inputs
            .to_dict()
            .at("oicana-inputs".into(), None)
            .expect("No inputs built!")
        else {
            panic!("Inputs should be dictionary!")
        };

        assert_eq!(inputs.len(), 2);
        assert!(inputs.contains("data"));
        assert!(inputs.contains("blob1"));
        assert!(!inputs.contains("blob2"));
    }

    #[test]
    fn sets_dev_compilation_mode() {
        let mut inputs = TemplateInputs::new();
        inputs.with_config(CompilationConfig::development());

        let Value::Dict(config) = inputs
            .to_dict()
            .at("oicana-config".into(), None)
            .expect("No config built!")
        else {
            panic!("Config should be dictionary!")
        };

        assert_eq!(
            config
                .at("production".into(), None)
                .expect("Mode should be in compilation config"),
            Value::Bool(false)
        );
    }

    #[test]
    fn sets_prod_compilation_mode() {
        let mut inputs = TemplateInputs::new();
        inputs.with_config(CompilationConfig::production());

        let Value::Dict(config) = inputs
            .to_dict()
            .at("oicana-config".into(), None)
            .expect("No config built!")
        else {
            panic!("Config should be dictionary!")
        };

        assert_eq!(
            config
                .at("production".into(), None)
                .expect("Mode should be in compilation config"),
            Value::Bool(true)
        );
    }

    #[test]
    fn sets_default_compilation_mode() {
        let inputs = TemplateInputs::new();

        let Value::Dict(config) = inputs
            .to_dict()
            .at("oicana-config".into(), None)
            .expect("No config built!")
        else {
            panic!("Config should be dictionary!")
        };

        assert_eq!(
            config
                .at("production".into(), None)
                .expect("Mode should be in compilation config"),
            Value::Bool(true)
        );
    }
}
