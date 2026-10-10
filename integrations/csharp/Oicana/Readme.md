# Oicana for C#

*Generate PDFs in .NET without a headless browser.*

In .NET, PDF generation usually means wrapping the abandoned wkhtmltopdf, running a headless browser next to your service, or writing every layout as code in a low-level library. All three make a simple invoice harder than it should be.

Oicana compiles PDFs in process through native bindings instead. You design documents as [Typst](https://typst.app/) templates, load them once at startup, and render them from JSON in single-digit milliseconds. No browser process, no per-document fees, no document data leaving your infrastructure.

> **Free for noncommercial use.** Commercial use is free for 30 days, then needs a [per-application subscription](https://oicana.com/#pricing) with unlimited seats.

## Installation

```bash
dotnet add package Oicana
```

Targets .NET 8.0 and newer. The package brings the native library for `linux-x64`, `linux-arm64`, `linux-musl-x64`, `linux-musl-arm64` (Alpine), `osx-x64`, `osx-arm64`, `win-x64`, and `win-arm64`. The glibc builds need glibc 2.28 or newer.

For smaller deployments, for example container images, reference `Oicana.Core` and only the native packages you deploy to instead:

```bash
dotnet add package Oicana.Core
dotnet add package Oicana.Native.linux-musl-x64
```

## Quick start

```csharp
using System.Text.Json.Nodes;
using Oicana;
using Oicana.Config;
using Oicana.Inputs;

var template = new Template(File.ReadAllBytes("invoice-0.1.0.zip"));

var jsonInputs = new Dictionary<string, JsonNode>
{
    ["invoice"] = JsonNode.Parse(
        """{ "number": "2026-001", "customer": "Acme GmbH", "total": "€1,190.00" }""")!,
};

var pdf = template.ExportPdf(jsonInputs);
```

`Export` returns a `Stream` you can hand to `Results.File` in a minimal API. `ExportPng` and `ExportSvg` produce the other formats, and `Template.ExportOnce` renders a one-off template without registering it.

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

## Running in a web service

Creating a `Template` compiles it once in development mode to warm up the Typst cache, so do it at startup, not per request. The instance is thread-safe and fits a singleton registration; afterwards there is no file I/O on the hot path.

## Why Oicana

- **Runs in your infrastructure**: PDFs are generated inside your own application. No data is sent to a third-party service.
- **Multi-platform**: the same template works in the browser, Node.js, C#, Java, Rust, Python, and PHP.
- **Powerful layouting**: templates have all of Typst, including its package ecosystem.
- **Performant**: a warmed up template renders a PDF in single-digit milliseconds.
- **AI and version control ready**: templates are text files. They live next to your code, and AI can help write them.
- **No proprietary format**: templates are plain Typst projects. The Typst compiler is open source.

## Where to go next

- [C# / ASP.NET getting started guide](https://oicana.com/docs/getting-started/4-2-csharp/): from an empty project to a PDF endpoint
- [Open source ASP.NET example](https://github.com/oicana/oicana-example-csharp-asp-net): blob inputs, error handling, and a preview endpoint
- [PDF generation in .NET](https://oicana.com/pdf-generation/csharp/): the shorter overview
- [How Oicana compares](https://oicana.com/compare/): against headless browsers, PDF libraries, and hosted APIs

## Licensing

Oicana is source-available under the [PolyForm Noncommercial License 1.0.0](https://github.com/oicana/oicana/blob/main/LICENSE.md) and free for noncommercial use. Commercial use is free for 30 days; see [pricing](https://oicana.com/#pricing) for subscriptions, or write to `hello@oicana.com`.
