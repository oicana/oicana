# Oicana for Node.js

*Generate PDFs in Node.js without a headless browser.*

The default answer in Node.js is Puppeteer: render HTML in headless Chrome and print it to PDF. That means shipping a full browser with your service, hundreds of megabytes of memory, slow cold starts, and CSS that was never designed for paged documents.

Oicana compiles PDFs in process through native bindings instead. You design documents as [Typst](https://typst.app/) templates, load them once at startup, and render them from JSON in single-digit milliseconds. No browser process, no per-document fees, no document data leaving your infrastructure.

> **Free for noncommercial use.** Commercial use is free for 30 days, then needs a [per-application subscription](https://oicana.com/#pricing) with unlimited seats.

## Installation

```bash
npm install @oicana/node
```

The matching native binary is installed automatically. Supported platforms: Linux x64 and arm64 (glibc and musl), macOS x64 and arm64, and Windows x64 and arm64.

## Quick start

```typescript
import { readFile } from 'node:fs/promises';
import { Template } from '@oicana/node';

const template = new Template(await readFile('invoice-0.1.0.zip'));

const jsonInputs = new Map<string, string>();
jsonInputs.set('invoice', JSON.stringify({
  number: '2026-001',
  customer: 'Acme GmbH',
  total: '€1,190.00',
}));

const pdf = template.export(jsonInputs, new Map());
```

`export` returns the PDF bytes as a `Uint8Array`. `exportPng` and `exportSvg` produce the other formats, and `Template.exportOnce` renders a one-off template without registering it.

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

## Keeping the event loop free

Compilation is CPU-bound, so every method has an async variant (`exportPdfAsync`, `compileAsync`, `Template.create`, and friends) that runs on Node.js' libuv thread pool instead of blocking the event loop. Under concurrent load that is the difference between a responsive service and a stalled one. The [async compilation guide](https://oicana.com/docs/guides/nodejs-async/) covers the async API and how to size the thread pool.

Load the template once at startup and export per request; afterwards there is no file I/O on the hot path.

## Why Oicana

- **Runs in your infrastructure**: PDFs are generated inside your own application. No data is sent to a third-party service.
- **Multi-platform**: the same template works in the browser, Node.js, C#, Java, Rust, Python, and PHP.
- **Powerful layouting**: templates have all of Typst, including its package ecosystem.
- **Performant**: a warmed up template renders a PDF in single-digit milliseconds.
- **AI and version control ready**: templates are text files. They live next to your code, and AI can help write them.
- **No proprietary format**: templates are plain Typst projects. The Typst compiler is open source.

## Where to go next

- [Node.js / NestJS getting started guide](https://oicana.com/docs/getting-started/4-4-nodejs/): from an empty project to a PDF endpoint
- [Open source NestJS example](https://github.com/oicana/oicana-example-typescript-nestjs): blob inputs, error handling, and Swagger docs
- [PDF generation in Node.js](https://oicana.com/pdf-generation/nodejs/): the shorter overview
- [How Oicana compares](https://oicana.com/compare/): against headless browsers, PDF libraries, and hosted APIs

## Licensing

Oicana is source-available under the [PolyForm Noncommercial License 1.0.0](https://github.com/oicana/oicana/blob/main/LICENSE.md) and free for noncommercial use. Commercial use is free for 30 days; see [pricing](https://oicana.com/#pricing) for subscriptions, or write to `hello@oicana.com`.
