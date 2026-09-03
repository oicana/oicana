//! The manifest of a template is available to the host.

use oicana::files::packed::PackedTemplate;
use oicana::input::input_definition::InputDefinition;
use oicana::template::PdfStandard;
use oicana::Template;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const MANIFEST: &str = r#"
[package]
name = "manifest-test"
version = "0.1.0"
entrypoint = "main.typ"

[tool.oicana]
manifest_version = 1

[[tool.oicana.inputs]]
type = "blob"
key = "default-blob"
default = { file = "default.txt", meta = { image_format = "png" } }

[[tool.oicana.inputs]]
type = "json"
key = "development-json"
development = "development.json"
schema = "input.schema.json"
"#;

fn packed_template() -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, content) in [
        ("typst.toml", MANIFEST),
        ("main.typ", "Content"),
        ("default.txt", "default"),
        ("development.json", "{}"),
        ("input.schema.json", "{}"),
    ] {
        writer.start_file(name, options).expect("start zip entry");
        writer
            .write_all(content.as_bytes())
            .expect("write zip entry");
    }

    writer.finish().expect("finish zip").into_inner()
}

#[test]
fn exposes_the_package_section_and_the_oicana_config() {
    let template =
        Template::<PackedTemplate>::init(Cursor::new(packed_template())).expect("init template");

    let manifest = template.manifest();

    assert_eq!(manifest.package.name, "manifest-test");
    assert_eq!(manifest.package.version.to_string(), "0.1.0");
    assert_eq!(manifest.tool.oicana.manifest_version, 1);
    assert!(manifest.tool.oicana.validate_json_inputs_by_default);
    assert_eq!(manifest.pdf_standards(), [PdfStandard::A_3b]);
    assert!(manifest.pdf_tagged());
    assert!(manifest.required_font_families().is_empty());

    let json = manifest
        .tool
        .oicana
        .inputs
        .iter()
        .find_map(|input| match input {
            InputDefinition::Json(json) if json.key == "development-json" => Some(json),
            _ => None,
        })
        .expect("the template declares the 'development-json' input");
    assert_eq!(json.schema.as_deref(), Some("input.schema.json"));
    assert_eq!(json.development.as_deref(), Some("development.json"));
    assert_eq!(json.default, None);
    assert!(json.validate);

    let blob = manifest
        .tool
        .oicana
        .inputs
        .iter()
        .find_map(|input| match input {
            InputDefinition::Blob(blob) if blob.key == "default-blob" => Some(blob),
            _ => None,
        })
        .expect("the template declares the 'default-blob' input");
    let default = blob.default.as_ref().expect("the input has a default blob");
    assert_eq!(default.file, "default.txt");
    assert_eq!(
        default
            .meta
            .as_ref()
            .and_then(|meta| meta.get("image_format"))
            .and_then(|format| format.as_str()),
        Some("png")
    );
    assert!(blob.development.is_none());
}
