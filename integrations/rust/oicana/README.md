# Oicana for Rust

*Generate PDFs from Typst templates, in process.*

Rust has solid low-level PDF crates, but hand-coding every table and page break gets old fast. And bundling a headless browser defeats the point of a lean Rust service.

Oicana is written in Rust, and this crate is the most direct way to use it. You design documents as [Typst](https://typst.app/) templates, load them once at startup, and render them from JSON in single-digit milliseconds. No browser process, no per-document fees, no document data leaving your infrastructure.

> **Free for noncommercial use.** Commercial use is free for 30 days, then needs a [per-application subscription](https://oicana.com/#pricing) with unlimited seats.

## Installation

```bash
cargo add oicana
```

Builds on stable Rust 1.88 or newer.

## Quick start

```rust
use std::fs::File;

use oicana::Template;
use oicana::export::pdf::export_pdf;
use oicana::input::JsonInput;
use oicana::input::{CompilationConfig, TemplateInputs};

let mut template = Template::init(File::open("invoice-0.1.0.zip")?)?;

let mut inputs = TemplateInputs::new();
inputs.with_config(CompilationConfig::production());
inputs.with_input(JsonInput::new(
    "invoice",
    serde_json::json!({
        "number": "2026-001",
        "customer": "Acme GmbH",
        "total": "€1,190.00"
    })
    .to_string(),
));

let document = template.compile(inputs)?;

let pdf = export_pdf(
    &document.document,
    &template,
    template.manifest().pdf_standards(),
    template.manifest().pdf_tagged(),
    None,
)?;
```
This also requires `serde_json` for the json input (run `cargo add serde_json`).

`compile` returns a reusable `CompiledDocument`, and the free `export_*` functions turn it into bytes.

`Template::init` only reads the template, it does not compile it, so the first `compile` call pays the full compilation cost. Call it once after `init` for the warm-up that the other integrations run at registration.

## What a template looks like

Templates are plain [Typst](https://typst.app/) projects. A `typst.toml` manifest names the entrypoint and declares the inputs your application passes in:

```toml
[package]
name = "invoice"
version = "0.1.0"
entrypoint = "main.typ"

[tool.oicana]
manifest_version = 1

[[tool.oicana.inputs]]
type = "json"
key = "invoice"
development = "invoice.json"
```

The entrypoint, `main.typ`, reads those inputs through the Oicana Typst package and lays out the document:

```typst
#import "@preview/oicana:0.2.0": setup

#let read-project-file(path) = read(path, encoding: none)
#let (input, oicana-image, oicana-config) = setup(read-project-file)

#set document(title: "Invoice", date: datetime.today())

= Invoice #input.invoice.number

Billed to: #input.invoice.customer

*Total: #input.invoice.total*
```

The `development` value lets the template preview with real data in any Typst editor. `oicana pack` turns the directory into `invoice-0.1.0.zip`, the archive every Oicana integration loads.

The [Oicana CLI](https://oicana.com/docs/cli/) does the packing, so a layout change ships as a new asset, not a code change.

## Feature flags

| Feature | Default | Purpose |
| ------- | ------- | ------- |
| `pdf` | yes | Export compiled documents to PDF |
| `png` | no | Export to PNG |
| `svg` | no | Export to SVG |
| `packed` | yes | Read templates from packed `.zip` archives |
| `native` | no | Read an unpacked template from disk, resolving Typst packages like the CLI |
| `preloaded` | no | Read a template from an in-memory file map (intended for tests) |

## Why Oicana

- **Runs in your infrastructure**: PDFs are generated inside your own application. No data is sent to a third-party service.
- **Multi-platform**: the same template works in the browser, Node.js, C#, Java, Rust, Python, and PHP.
- **Powerful layouting**: templates have all of Typst, including its package ecosystem.
- **Performant**: a warmed up template renders a PDF in single-digit milliseconds.
- **AI and version control ready**: templates are text files. They live next to your code, and AI can help write them.
- **No proprietary format**: templates are plain Typst projects. The Typst compiler is open source.

## Where to go next

- [Rust / Axum getting started guide](https://oicana.com/docs/getting-started/4-5-rust/): from an empty project to a PDF endpoint
- [Open source Axum example](https://github.com/oicana/oicana-example-rust-axum): thread-safe template caching with `DashMap`
- [PDF generation in Rust](https://oicana.com/pdf-generation/rust/): the shorter overview
- [How Oicana compares](https://oicana.com/compare/): against headless browsers, PDF libraries, and hosted APIs

## Licensing

Oicana is source-available under the [PolyForm Noncommercial License 1.0.0](https://github.com/oicana/oicana/blob/main/LICENSE.md) and free for noncommercial use. Commercial use is free for 30 days; see [pricing](https://oicana.com/#pricing) for subscriptions, or write to `hello@oicana.com`.
