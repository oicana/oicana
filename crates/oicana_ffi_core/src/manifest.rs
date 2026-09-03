//! The template manifest in the shape the integrations expose it.
//!
//! Every integration declares these structures in its own language.
//! Keys are camelCase, optional values are always present as `null`
//! and lists are always present, so a wrapper can rely on every key
//! existing.

use oicana_input::input_definition::blob::BlobInputDefinition as CoreBlobInputDefinition;
use oicana_input::input_definition::json::JsonInputDefinition as CoreJsonInputDefinition;
use oicana_input::input_definition::InputDefinition as CoreInputDefinition;
use oicana_template::manifest::TemplateManifest;
use oicana_template::OicanaConfig as CoreOicanaConfig;
use serde::Serialize;

/// A template's manifest.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// The Typst package section of the manifest.
    pub package: Package,
    /// The Oicana section of the manifest.
    pub oicana: OicanaConfig,
}

/// The Typst package a template is.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    /// Name of the template.
    pub name: String,
    /// Version of the template.
    pub version: String,
    /// File the compilation starts at.
    pub entrypoint: String,
    /// Authors of the template.
    pub authors: Vec<String>,
    /// License of the template.
    pub license: Option<String>,
    /// Short description of the template.
    pub description: Option<String>,
    /// Web presence of the template.
    pub homepage: Option<String>,
    /// Repository the template is developed in.
    pub repository: Option<String>,
}

/// The Oicana configuration of a template.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OicanaConfig {
    /// Version of the manifest format.
    pub manifest_version: u8,
    /// The inputs the template declares, in manifest order.
    pub inputs: Vec<InputDefinition>,
    /// Whether JSON inputs are validated against their schemas by default.
    pub validate_json_inputs_by_default: bool,
    /// How compiled documents are exported.
    pub export: ExportConfig,
    /// Fonts the template expects from its host.
    pub fonts: FontConfig,
}

/// An input a template declares.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum InputDefinition {
    /// An input taking a JSON value.
    #[serde(rename = "json")]
    Json(JsonInputDefinition),
    /// An input taking arbitrary bytes.
    #[serde(rename = "blob")]
    Blob(BlobInputDefinition),
}

/// An input taking a JSON value.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JsonInputDefinition {
    /// Key the input is supplied and used under.
    pub key: String,
    /// Whether a value of this input is required for compilation.
    pub required: bool,
    /// File in the template holding the value used when none is supplied.
    ///
    /// In development mode, [`Self::development`] takes precedence.
    pub default: Option<String>,
    /// File in the template holding the value used in development mode when none is supplied.
    pub development: Option<String>,
    /// File in the template holding the JSON schema of this input.
    pub schema: Option<String>,
    /// Whether values are validated against [`Self::schema`].
    pub validate: bool,
}

/// An input taking arbitrary bytes.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlobInputDefinition {
    /// Key the input is supplied and used under.
    pub key: String,
    /// Whether a value of this input is required for compilation.
    pub required: bool,
    /// Blob used when no value is supplied.
    ///
    /// In development mode, [`Self::development`] takes precedence.
    pub default: Option<BlobFallback>,
    /// Blob used in development mode when no value is supplied.
    pub development: Option<BlobFallback>,
}

/// A blob from the template, used when no value is supplied.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlobFallback {
    /// File in the template holding the blob.
    pub file: String,
    /// Metadata passed to the template along with the blob.
    pub meta: Option<serde_json::Value>,
}

/// How compiled documents are exported.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportConfig {
    /// PDF export configuration.
    pub pdf: PdfExportConfig,
}

/// How documents are exported to PDF.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PdfExportConfig {
    /// PDF standards the export conforms to, for example `a-3b`.
    pub standards: Vec<String>,
    /// Whether the PDF is tagged for accessibility.
    pub tagged: bool,
}

/// Fonts a template expects from its host.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FontConfig {
    /// Font families the host has to register.
    pub require: Vec<String>,
}

impl From<&TemplateManifest> for Manifest {
    fn from(manifest: &TemplateManifest) -> Self {
        Manifest {
            package: Package {
                name: manifest.package.name.to_string(),
                version: manifest.package.version.to_string(),
                entrypoint: manifest.package.entrypoint.to_string(),
                authors: manifest
                    .package
                    .authors
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                license: manifest.package.license.as_ref().map(ToString::to_string),
                description: manifest
                    .package
                    .description
                    .as_ref()
                    .map(ToString::to_string),
                homepage: manifest.package.homepage.as_ref().map(ToString::to_string),
                repository: manifest
                    .package
                    .repository
                    .as_ref()
                    .map(ToString::to_string),
            },
            oicana: (&manifest.tool.oicana).into(),
        }
    }
}

impl From<&CoreOicanaConfig> for OicanaConfig {
    fn from(config: &CoreOicanaConfig) -> Self {
        OicanaConfig {
            manifest_version: config.manifest_version,
            inputs: config.inputs.iter().map(InputDefinition::from).collect(),
            validate_json_inputs_by_default: config.validate_json_inputs_by_default,
            export: ExportConfig {
                pdf: PdfExportConfig {
                    standards: config
                        .export
                        .pdf
                        .standards
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                    tagged: config.export.pdf.tagged,
                },
            },
            fonts: FontConfig {
                require: config.fonts.require.clone(),
            },
        }
    }
}

impl From<&CoreInputDefinition> for InputDefinition {
    fn from(definition: &CoreInputDefinition) -> Self {
        match definition {
            CoreInputDefinition::Json(json) => InputDefinition::Json(json.into()),
            CoreInputDefinition::Blob(blob) => InputDefinition::Blob(blob.into()),
        }
    }
}

impl From<&CoreJsonInputDefinition> for JsonInputDefinition {
    fn from(definition: &CoreJsonInputDefinition) -> Self {
        JsonInputDefinition {
            key: definition.key.clone(),
            required: definition.required,
            default: definition.default.clone(),
            development: definition.development.clone(),
            schema: definition.schema.clone(),
            validate: definition.validate,
        }
    }
}

impl From<&CoreBlobInputDefinition> for BlobInputDefinition {
    fn from(definition: &CoreBlobInputDefinition) -> Self {
        BlobInputDefinition {
            key: definition.key.clone(),
            required: definition.required,
            default: definition.default.as_ref().map(BlobFallback::from),
            development: definition.development.as_ref().map(BlobFallback::from),
        }
    }
}

impl From<&oicana_input::input_definition::blob::FallbackBlobInput> for BlobFallback {
    fn from(fallback: &oicana_input::input_definition::blob::FallbackBlobInput) -> Self {
        BlobFallback {
            file: fallback.file.clone(),
            meta: fallback
                .meta
                .as_ref()
                .and_then(|meta| serde_json::to_value(meta).ok()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Manifest;
    use oicana_template::manifest::TemplateManifest;

    #[test]
    fn serializes_every_field_of_a_maximal_manifest() {
        let manifest = TemplateManifest::from_toml(
            r#"
            [package]
            name = "invoice"
            version = "1.2.3"
            entrypoint = "main.typ"
            authors = ["Oicana <hello@oicana.com>"]
            license = "MIT"
            description = "An invoice."
            homepage = "https://oicana.com"
            repository = "https://github.com/oicana/oicana"
            keywords = ["invoice"]

            [tool.oicana]
            manifest_version = 1
            validate_json_inputs_by_default = false
            tests = "custom_tests"

            [tool.oicana.export.pdf]
            standards = ["2.0", "a-4"]
            tagged = false

            [tool.oicana.fonts]
            require = ["Oicana Test", "Inria Serif"]

            [[tool.oicana.inputs]]
            type = "json"
            key = "invoice"
            required = false
            development = "dev.json"
            schema = "invoice.schema.json"
            validate = false

            [[tool.oicana.inputs]]
            type = "blob"
            key = "logo"
            default = { file = "logo.png", meta = { image_format = "png", dpi = 300, tags = ["a", "b"], nested = { deep = true } } }
            development = { file = "dev-logo.png" }
            "#,
        )
        .expect("the manifest parses");

        let serialized =
            serde_json::to_value(Manifest::from(&manifest)).expect("the manifest serializes");

        assert_eq!(
            serialized,
            serde_json::json!({
                "package": {
                    "name": "invoice",
                    "version": "1.2.3",
                    "entrypoint": "main.typ",
                    "authors": ["Oicana <hello@oicana.com>"],
                    "license": "MIT",
                    "description": "An invoice.",
                    "homepage": "https://oicana.com",
                    "repository": "https://github.com/oicana/oicana"
                },
                "oicana": {
                    "manifestVersion": 1,
                    "inputs": [
                        {
                            "type": "json",
                            "key": "invoice",
                            "required": false,
                            "default": null,
                            "development": "dev.json",
                            "schema": "invoice.schema.json",
                            "validate": false
                        },
                        {
                            "type": "blob",
                            "key": "logo",
                            "required": true,
                            "default": {
                                "file": "logo.png",
                                "meta": {
                                    "image_format": "png",
                                    "dpi": 300,
                                    "tags": ["a", "b"],
                                    "nested": { "deep": true }
                                }
                            },
                            "development": { "file": "dev-logo.png", "meta": null }
                        }
                    ],
                    "validateJsonInputsByDefault": false,
                    "export": {
                        "pdf": { "standards": ["2.0", "a-4"], "tagged": false }
                    },
                    "fonts": { "require": ["Oicana Test", "Inria Serif"] }
                }
            })
        );
    }
}
