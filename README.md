# Oicana

*One template. Every platform. Typeset PDFs.*

[https://oicana.com](https://oicana.com)

Oicana compiles [Typst](https://typst.app/) templates to PDF, PNG, and SVG from Node.js, Python, Java, C#, Rust, PHP, Go, Ruby, Dart, and the browser. No headless Chrome, no per-document fees, no document data leaving your infrastructure. One template format works everywhere.

> **Free for noncommercial use.** Commercial use is free for 30 days, then needs a [per-application subscription](https://oicana.com/#pricing) with unlimited seats.

## Why Oicana

- **Runs in your infrastructure**: PDFs are generated inside your own application. No data is sent to a third-party service.
- **Multi-platform**: the same template works in the browser, Node.js, C#, Java, Rust, Python, PHP, Go, Ruby, and Dart.
- **Powerful layouting**: templates have all of Typst, including its package ecosystem.
- **Performant**: a warmed up template renders a PDF in single-digit milliseconds.
- **AI and version control ready**: templates are text files. They live next to your code, and AI can help write them.
- **No proprietary format**: templates are plain Typst projects. The Typst compiler is open source.

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
#import "@preview/oicana:0.2.1": setup

#let read-project-file(path) = read(path, encoding: none)
#let (input, oicana-image, oicana-config) = setup(read-project-file)

#set document(title: "Invoice", date: datetime.today())

= Invoice #input.invoice.number

Billed to: #input.invoice.customer

*Total: #input.invoice.total*
```

The `development` value lets the template preview with real data in any Typst editor. `oicana pack` turns the directory into `invoice-0.1.0.zip`, the archive every Oicana integration loads.

## Integrations

| Platform | Package | Guide |
| -------- | ------- | ----- |
| Browser | [`@oicana/browser`](https://www.npmjs.com/package/@oicana/browser) | [Browser / React](https://oicana.com/docs/getting-started/4-1-browser/) |
| Node.js | [`@oicana/node`](https://www.npmjs.com/package/@oicana/node) | [Node.js / NestJS](https://oicana.com/docs/getting-started/4-4-nodejs/) |
| C# | [`Oicana`](https://www.nuget.org/packages/Oicana) | [C# / ASP.NET](https://oicana.com/docs/getting-started/4-2-csharp/) |
| Java | [`com.oicana:oicana`](https://central.sonatype.com/artifact/com.oicana/oicana) | [Java / Spring Boot](https://oicana.com/docs/getting-started/4-3-java/) |
| Rust | [`oicana`](https://crates.io/crates/oicana) | [Rust / Axum](https://oicana.com/docs/getting-started/4-5-rust/) |
| Python | [`oicana`](https://pypi.org/project/oicana/) | [Python / FastAPI](https://oicana.com/docs/getting-started/4-6-python/) |
| PHP | [`oicana/oicana`](https://composer.oicana.com) | [PHP / Slim](https://oicana.com/docs/getting-started/4-7-php/) |
| Go | [`go.oicana.com/oicana`](https://go.oicana.com) | [Go / net/http](https://oicana.com/docs/getting-started/4-8-go/) |
| Ruby | [`oicana`](https://rubygems.org/gems/oicana) | [Ruby / Rails](https://oicana.com/docs/getting-started/4-9-ruby/) |
| Dart / Flutter | [`oicana`](https://pub.dev/packages/oicana) | [Dart / Flutter](https://oicana.com/docs/getting-started/4-10-dart/) |

Every integration has an open source example project in the [Oicana GitHub organization](https://github.com/oicana). The browser example is deployed at [example.oicana.com](https://example.oicana.com).

> More integrations are planned. Missing one? Open a GitHub issue or write to `hello@oicana.com`. It helps us prioritize.

## Getting started

The [getting started guide](https://oicana.com/docs/getting-started/1-setup/) walks through creating a template, defining its inputs, and generating a PDF from any integration.

Comparing options? Read [how Oicana differs from headless browsers, PDF libraries, LaTeX, and hosted APIs](https://oicana.com/compare/).

## Template development

An Oicana template consists of

- one or more Typst `.typ` files
- a `typst.toml` manifest with `name`, `version`, `entrypoint`, and `tool.oicana.manifest_version`, plus any number of input definitions

Templates are ordinary Typst projects, so you can edit them in the official [Typst editor](https://typst.app/) or any editor with Typst support. Start from the [open source example templates](https://github.com/oicana/oicana-example-templates).

### Typst package

Every template sets up the [Oicana Typst package](https://typst.app/universe/package/oicana). It collects the declared inputs and falls back to their `default` or `development` values:

```typst
#import "@preview/oicana:0.2.1": setup

#let read-project-file(path) = read(path, encoding: none)
#let (input, oicana-image, oicana-config) = setup(read-project-file)
```

### PDF standards

A template declares the standards its PDFs conform to and whether the output is tagged for accessibility:

```toml
[tool.oicana.export.pdf]
standards = ["2.0", "a-4"]
tagged = true
```

| Family | Accepted values |
| ------ | --------------- |
| Base PDF version | `1.4`, `1.5`, `1.6`, `1.7`, `2.0` |
| PDF/A (archival) | `a-1b`, `a-1a`, `a-2b`, `a-2u`, `a-2a`, `a-3b`, `a-3u`, `a-3a`, `a-4`, `a-4f`, `a-4e` |
| PDF/UA (accessibility) | `ua-1` |

Standards combine as long as the combination is producible: at most one base version, at most one PDF/A standard, and at most one PDF/UA standard, all sharing overlapping PDF versions. `oicana validate` rejects the rest.

The defaults are a plain PDF without a standard and `tagged = true`. Tagging is skipped automatically when a page range omits pages, because Typst cannot tag a partial document.

On top of comparing snapshots, `oicana test` exports every test document under the configured standards, so a template that cannot be produced in its declared standard fails the suite. Set `pdf = false` on a single test or a whole collection to skip the export.

### CLI

The CLI scaffolds, validates, packages, and tests templates. See the [CLI documentation](https://oicana.com/docs/cli/) for installation and the full reference.

```bash
oicana new invoice                          # scaffold a template
oicana validate -a                          # check every manifest, input schema, and fallback value
oicana compile -j invoice=invoice.json      # render an unpacked template
oicana watch                                # recompile whenever a source file changes
oicana test -a                              # snapshot tests, plus PDF export in the declared standards
oicana pack --all                           # produce the archives integrations load
```

Integrations load packed archives, not directories, so `oicana pack` is the one command every project needs.

## Pronunciation

/ɔɪkɑna/

## Licensing

Oicana is source-available under the [PolyForm Noncommercial License 1.0.0](https://github.com/oicana/oicana/blob/main/LICENSE.md) and free for noncommercial use. Commercial use is free for 30 days; see [pricing](https://oicana.com/#pricing) for subscriptions, or write to `hello@oicana.com`.

The [Typst integration](https://typst.app/universe/package/oicana) and the example projects in the Oicana GitHub organization are open source under their respective licenses. See [NOTICE](NOTICE) for a report of the third party licenses in this project.
