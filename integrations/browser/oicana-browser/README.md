# Oicana for the browser

*Generate PDFs in the browser, no server involved.*

Client-side PDF generation usually means drawing every line yourself with a low-level library, or sending user data to a backend or third-party API. The first buries your document design in code, the second sends the data off the device.

Oicana runs the [Typst](https://typst.app/) compiler as WebAssembly in the browser. You design documents as templates, ship them as static assets, and compile them to PDF, PNG, or SVG from JSON. On your users' devices, no data egress, no per-document fees.

> **Free for noncommercial use.** Commercial use is free for 30 days, then needs a [per-application subscription](https://oicana.com/#pricing) with unlimited seats.

## Installation

```bash
npm install @oicana/browser
```

## Quick start

```typescript
import { Template, initialize } from '@oicana/browser';
import wasmUrl from '@oicana/browser/oicana.wasm?url';

await initialize(wasmUrl);

const templateFile = await fetch('/invoice-0.1.0.zip');
const template = new Template(new Uint8Array(await templateFile.arrayBuffer()));

const jsonInputs = new Map<string, string>();
jsonInputs.set('invoice', JSON.stringify({
  number: '2026-001',
  customer: 'Acme GmbH',
  total: '€1,190.00',
}));

const pdf = template.export(jsonInputs, new Map());
```

`export` returns the PDF bytes as a `Uint8Array`, ready for a `Blob` to download or preview. `exportPng` and `exportSvg` produce the other formats, and `exportOnce` renders a one-off template without registering it.

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

The [Oicana CLI](https://oicana.com/docs/cli/) does the packing. Served as a static asset, a template can change without redeploying your application.

## Running in a Web Worker

Template compilation is CPU-bound and runs synchronously, so it blocks the thread it is called on. For anything beyond small templates, run this package inside a [Web Worker](https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API) to keep the main thread and your UI responsive.

## Shipping the WASM file

The WASM module is about 40 MB uncompressed (~17 MB gzipped, ~12 MB brotli). It is fetched once and browser-cached. Many CDNs silently skip on-the-fly compression above ~10 MB, so pre-compressing it matters. The [browser deployment guide](https://oicana.com/docs/guides/browser-deployment/) covers pre-compression, CDN caveats, and Web Worker offload.

## Why Oicana

- **Runs in your infrastructure**: PDFs are generated inside your own application. No data is sent to a third-party service.
- **Multi-platform**: the same template works in the browser, Node.js, C#, Java, Rust, Python, and PHP.
- **Powerful layouting**: templates have all of Typst, including its package ecosystem.
- **Performant**: a warmed up template renders a PDF in single-digit milliseconds.
- **AI and version control ready**: templates are text files. They live next to your code, and AI can help write them.
- **No proprietary format**: templates are plain Typst projects. The Typst compiler is open source.

## Where to go next

- [Browser / React getting started guide](https://oicana.com/docs/getting-started/4-1-browser/): from an empty project to a downloadable PDF
- [Open source React example](https://github.com/oicana/oicana-example-typescript-react): deployed at [example.oicana.com](https://example.oicana.com)
- [PDF generation in the browser](https://oicana.com/pdf-generation/browser/): the shorter overview
- [How Oicana compares](https://oicana.com/compare/): against headless browsers, PDF libraries, and hosted APIs

## Licensing

Oicana is source-available under the [PolyForm Noncommercial License 1.0.0](https://github.com/oicana/oicana/blob/main/LICENSE.md) and free for noncommercial use. Commercial use is free for 30 days; see [pricing](https://oicana.com/#pricing) for subscriptions, or write to `hello@oicana.com`.
