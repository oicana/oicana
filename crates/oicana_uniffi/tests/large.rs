//! A 100 MB blob round-trip called directly from Rust, as the baseline for measuring
//! integrations. Ignored by default; run it in release mode:
//! `cargo test --release -p oicana_uniffi --test large -- --ignored --nocapture`

use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::time::Instant;

use oicana_uniffi::{BlobInput, CompilationMode, ExportFormat, Template};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const SIZE: usize = 100 * 1024 * 1024;

fn payload_template() -> Vec<u8> {
    let manifest =
        "[package]\nname = \"uniffi-large\"\nversion = \"0.1.0\"\nentrypoint = \"main.typ\"\n\n\
        [tool.oicana]\nmanifest_version = 1\n\n\
        [[tool.oicana.inputs]]\ntype = \"blob\"\nkey = \"payload\"\nrequired = false\n";
    let main = "#set document(date: none)\n\
        #let payload = sys.inputs.at(\"oicana-inputs\", default: (:)).at(\"payload\", default: none)\n\
        #if payload != none { pdf.attach(\"payload.bin\", payload.bytes) }\n";
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, content) in [("typst.toml", manifest), ("main.typ", main)] {
        writer.start_file(name, options).unwrap();
        writer.write_all(content.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// Incompressible bytes, so the PDF attachment stays as large as the input.
fn noise(len: usize) -> Vec<u8> {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect()
}

/// Peak resident memory in MiB, where the platform reports it.
fn peak_mib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib / 1024)
}

#[test]
#[ignore = "measurement, run explicitly in release mode"]
fn large_round_trip() {
    let data = noise(SIZE);
    let template = Template::new(
        payload_template(),
        HashMap::new(),
        HashMap::new(),
        CompilationMode::Development,
        None,
    )
    .unwrap();
    let baseline = peak_mib();

    let start = Instant::now();
    let document = template
        .compile(
            HashMap::new(),
            HashMap::from([(
                "payload".to_owned(),
                BlobInput {
                    data,
                    metadata: "{}".to_owned(),
                },
            )]),
            CompilationMode::Production,
        )
        .unwrap();
    let compiled = start.elapsed();
    let pdf = document.export(ExportFormat::Pdf, None).unwrap();
    let exported = start.elapsed() - compiled;

    println!(
        "100 MB round-trip: output {} bytes, compile {compiled:?}, export {exported:?}, \
         peak {:?} MiB (baseline {baseline:?} MiB)",
        pdf.len(),
        peak_mib()
    );
    assert!(pdf.len() >= SIZE);
}
