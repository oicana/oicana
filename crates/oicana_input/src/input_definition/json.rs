use serde::{Deserialize, Serialize};

/// An input for JSON values.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct JsonInputDefinition {
    /// The key of the input.
    ///
    /// Use this in the Typst code to refer to the current value of the input.
    pub key: String,
    /// Whether this input must have a value when compiling the template.
    ///
    /// Defaults to `true`. When `true`, compilation will fail if no value
    /// is supplied and no default or development value is configured.
    #[serde(default = "default_true")]
    pub required: bool,
    /// Path to a file used as default value for this input in case no other value is supplied.
    ///
    /// During development, the value of [`Self::development`] is preferred.
    pub default: Option<String>,
    /// Path to a file used as input value during development, when no value is supplied.
    pub development: Option<String>,
    /// Path to a JSON schema to validate input against.
    pub schema: Option<String>,
    /// Whether to validate this input against its schema.
    ///
    /// Defaults to `true`. Set to `false` to skip validation for this input
    /// even if a schema is defined. When `false`, no validator is compiled
    /// for this input during template initialization.
    #[serde(default = "default_true")]
    pub validate: bool,
}

fn default_true() -> bool {
    true
}
