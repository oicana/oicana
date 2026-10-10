//! Tests for the exported UniFFI API.

use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::Arc;
use std::thread;

use oicana_uniffi::{
    export_once, register_fonts, version, BlobInput, CompilationMode, ExportFormat, OicanaError,
    PageRange, Template, ZipLimits,
};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const PAYLOAD_TEMPLATE: &str = r#"#set document(date: none)
#let inputs = sys.inputs.at("oicana-inputs", default: (:))
#let payload = inputs.at("payload", default: none)
#if payload != none {
  pdf.attach("payload.bin", payload.bytes)
  [Payload of #payload.bytes.len() bytes]
} else [No payload]
"#;

fn template_zip(inputs: &str, main_typst: &str) -> Vec<u8> {
    let manifest = format!(
        "[package]\nname = \"uniffi-test\"\nversion = \"0.1.0\"\nentrypoint = \"main.typ\"\n\n\
         [tool.oicana]\nmanifest_version = 1\n{inputs}"
    );
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, content) in [("typst.toml", manifest.as_str()), ("main.typ", main_typst)] {
        writer.start_file(name, options).unwrap();
        writer.write_all(content.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn payload_template() -> Vec<u8> {
    template_zip(
        "\n[[tool.oicana.inputs]]\ntype = \"blob\"\nkey = \"payload\"\nrequired = false\n",
        PAYLOAD_TEMPLATE,
    )
}

fn payload(data: Vec<u8>) -> HashMap<String, BlobInput> {
    HashMap::from([(
        "payload".to_owned(),
        BlobInput {
            data,
            metadata: "{}".to_owned(),
        },
    )])
}

fn register(files: Vec<u8>) -> Arc<Template> {
    Template::new(
        files,
        HashMap::new(),
        HashMap::new(),
        CompilationMode::Development,
        None,
    )
    .unwrap()
}

fn repo_file(path: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path),
    )
    .unwrap()
}

#[test]
fn compiles_and_exports_every_format() {
    let template = register(payload_template());
    let document = template
        .compile(
            HashMap::new(),
            payload(vec![1, 2, 3]),
            CompilationMode::Production,
        )
        .unwrap();

    assert!(document
        .export(ExportFormat::Pdf, None)
        .unwrap()
        .starts_with(b"%PDF-"));
    assert!(document
        .export(ExportFormat::Png { pixels_per_pt: 1.0 }, None)
        .unwrap()
        .starts_with(b"\x89PNG"));
    let svg = String::from_utf8(document.export(ExportFormat::Svg, None).unwrap()).unwrap();
    assert!(svg.starts_with("<svg"));

    let pages = document.pages();
    assert_eq!(pages.len(), 1);
    assert!(pages[0].width > 0.0 && pages[0].height > 0.0);
    assert_eq!(document.warnings(), None);
}

#[test]
fn matches_oicana_ffi_core_output() {
    let files = repo_file("e2e-tests/template/oicana-e2e-test-x.y.z.zip");
    let json = HashMap::from([(
        "development-json".to_owned(),
        String::from_utf8(repo_file("assets/inputs/input.json")).unwrap(),
    )]);
    let meta = r#"{"image_format":"jpeg","foo":43,"bar":["input","two"]}"#;
    let blob = repo_file("assets/inputs/input.txt");

    let core = oicana_ffi_core::export_once(
        &files,
        json.clone(),
        HashMap::from([(
            "development-blob".to_owned(),
            oicana_ffi_core::BlobWithMetadata {
                bytes: blob.clone(),
                meta: meta.to_owned(),
            },
        )]),
        oicana_ffi_core::CompilationMode::Production,
        oicana_ffi_core::ExportFormat::Pdf,
        None,
        None,
    )
    .unwrap();

    let blobs = HashMap::from([(
        "development-blob".to_owned(),
        BlobInput {
            data: blob,
            metadata: meta.to_owned(),
        },
    )]);
    let once = export_once(
        files.clone(),
        json.clone(),
        blobs.clone(),
        CompilationMode::Production,
        ExportFormat::Pdf,
        None,
        None,
    )
    .unwrap();
    let compiled = register(files)
        .compile(json, blobs, CompilationMode::Production)
        .unwrap()
        .export(ExportFormat::Pdf, None)
        .unwrap();

    assert_eq!(once.bytes, core.bytes);
    assert_eq!(compiled, core.bytes);
    assert_eq!(once.warnings, core.warnings);
}

#[test]
fn exposes_template_files_and_manifest() {
    let template = register(payload_template());

    assert!(template.manifest().unwrap().contains("\"payload\""));
    assert_eq!(
        template.source("main.typ".into()).unwrap(),
        PAYLOAD_TEMPLATE
    );
    assert_eq!(
        template.file("main.typ".into()).unwrap(),
        PAYLOAD_TEMPLATE.as_bytes()
    );
    template.set_validate_inputs(false).unwrap();
}

#[test]
fn documents_outlive_their_template() {
    let template = register(payload_template());
    let document = template
        .compile(HashMap::new(), HashMap::new(), CompilationMode::Production)
        .unwrap();
    drop(template);

    assert!(document.export(ExportFormat::Pdf, None).is_ok());
}

#[test]
fn page_ranges_select_pages() {
    let template = register(template_zip("", "One #pagebreak() Two #pagebreak() Three"));
    let document = template
        .compile(HashMap::new(), HashMap::new(), CompilationMode::Production)
        .unwrap();
    assert_eq!(document.pages().len(), 3);

    let svg = |pages| document.export(ExportFormat::Svg, pages).unwrap();
    let single = svg(Some(PageRange {
        start: Some(1),
        end: Some(1),
    }));
    assert!(single.len() < svg(None).len());
    assert_eq!(
        single,
        svg(Some(PageRange {
            start: Some(1),
            end: Some(1)
        }))
    );
}

#[test]
fn warnings_are_reported() {
    let template = register(template_zip("", "#set text(font: \"No Such Font\")\nHi"));
    assert!(template.warnings().is_some());

    let document = template
        .compile(HashMap::new(), HashMap::new(), CompilationMode::Production)
        .unwrap();
    assert!(document.warnings().unwrap().contains("No Such Font"));
}

#[test]
fn an_invalid_archive_is_an_invalid_template() {
    let error = Template::new(
        b"not a zip".to_vec(),
        HashMap::new(),
        HashMap::new(),
        CompilationMode::Production,
        None,
    )
    .err()
    .unwrap();
    assert!(matches!(error, OicanaError::InvalidTemplate(_)), "{error}");
}

#[test]
fn zip_limits_are_enforced() {
    let error = Template::new(
        payload_template(),
        HashMap::new(),
        HashMap::new(),
        CompilationMode::Production,
        Some(ZipLimits {
            max_entries: Some(1),
            max_total_decompressed_bytes: None,
        }),
    )
    .err()
    .unwrap();
    assert!(matches!(error, OicanaError::InvalidTemplate(_)), "{error}");
}

#[test]
fn a_failing_warm_up_carries_the_diagnostics() {
    let error = Template::new(
        template_zip("", "#panic(\"broken on purpose\")"),
        HashMap::new(),
        HashMap::new(),
        CompilationMode::Production,
        None,
    )
    .err()
    .unwrap();
    assert!(matches!(error, OicanaError::Compilation(_)), "{error}");
    assert!(error.to_string().contains("broken on purpose"), "{error}");
}

#[test]
fn a_failing_compilation_carries_the_diagnostics() {
    let template = register(template_zip(
        "\n[[tool.oicana.inputs]]\ntype = \"json\"\nkey = \"fail\"\nrequired = false\n",
        "#if \"fail\" in sys.inputs.at(\"oicana-inputs\", default: (:)) { panic(\"failed on request\") }",
    ));
    let error = template
        .compile(
            HashMap::from([("fail".to_owned(), "true".to_owned())]),
            HashMap::new(),
            CompilationMode::Production,
        )
        .err()
        .unwrap();
    assert!(matches!(error, OicanaError::Compilation(_)), "{error}");
    assert!(error.to_string().contains("failed on request"), "{error}");
}

#[test]
fn invalid_inputs_are_rejected() {
    let template = register(payload_template());

    let undeclared = template
        .compile(
            HashMap::from([("unknown".to_owned(), "1".to_owned())]),
            HashMap::new(),
            CompilationMode::Production,
        )
        .err()
        .unwrap();
    assert!(
        matches!(undeclared, OicanaError::InvalidInput(_)),
        "{undeclared}"
    );

    let bad_meta = template
        .compile(
            HashMap::new(),
            HashMap::from([(
                "payload".to_owned(),
                BlobInput {
                    data: vec![],
                    metadata: "not json".to_owned(),
                },
            )]),
            CompilationMode::Production,
        )
        .err()
        .unwrap();
    assert!(
        matches!(bad_meta, OicanaError::InvalidInput(_)),
        "{bad_meta}"
    );
}

#[test]
fn missing_files_and_pages_are_reported() {
    let template = register(payload_template());
    let missing = template.source("missing.typ".into()).unwrap_err();
    assert!(matches!(missing, OicanaError::FileNotFound(_)), "{missing}");
    let missing = template.file("missing.bin".into()).unwrap_err();
    assert!(matches!(missing, OicanaError::FileNotFound(_)), "{missing}");

    let document = template
        .compile(HashMap::new(), HashMap::new(), CompilationMode::Production)
        .unwrap();
    let out_of_range = document
        .export(
            ExportFormat::Pdf,
            Some(PageRange {
                start: Some(5),
                end: None,
            }),
        )
        .unwrap_err();
    assert!(
        matches!(out_of_range, OicanaError::Export(_)),
        "{out_of_range}"
    );
}

#[test]
fn unreadable_fonts_are_skipped() {
    assert_eq!(register_fonts(vec![b"not a font".to_vec()]), 0);
}

#[test]
fn version_is_the_crate_version() {
    assert_eq!(version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn compiles_from_eight_threads_at_once() {
    let shared = register(payload_template());
    let expected = shared
        .compile(
            HashMap::new(),
            payload(vec![9; 64]),
            CompilationMode::Production,
        )
        .unwrap()
        .export(ExportFormat::Pdf, None)
        .unwrap();

    let outputs: Vec<(Vec<u8>, Vec<u8>)> = thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let shared = Arc::clone(&shared);
                scope.spawn(move || {
                    let from_shared = shared
                        .compile(
                            HashMap::new(),
                            payload(vec![9; 64]),
                            CompilationMode::Production,
                        )
                        .unwrap()
                        .export(ExportFormat::Pdf, None)
                        .unwrap();
                    let own = register(payload_template());
                    let from_own = own
                        .compile(
                            HashMap::new(),
                            payload(vec![9; 64]),
                            CompilationMode::Production,
                        )
                        .unwrap()
                        .export(ExportFormat::Pdf, None)
                        .unwrap();
                    (from_shared, from_own)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    for (from_shared, from_own) in outputs {
        assert_eq!(from_shared, expected);
        assert_eq!(from_own, expected);
    }
}
