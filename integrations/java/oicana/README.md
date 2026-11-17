# Oicana for Java

*Generate PDFs on the JVM without a headless browser.*

On the JVM, PDF generation usually means low-level libraries where every table and page break is layout code, or rendering HTML in a headless browser next to your service. One buries the document design in Java, the other adds a heavy runtime dependency.

Oicana compiles PDFs in process through a native JNI interface instead. You design documents as [Typst](https://typst.app/) templates, load them once at startup, and render them from JSON in single-digit milliseconds. No browser process, no per-document fees, no document data leaving your infrastructure.

> **Free for noncommercial use.** Commercial use is free for 30 days, then needs a [per-application subscription](https://oicana.com/#pricing) with unlimited seats.

## Installation

Add the API artifact plus the native artifact for every platform you target. Requires Java 17 or newer.

```kotlin
dependencies {
    implementation("com.oicana:oicana:<version>")

    runtimeOnly("com.oicana:oicana-linux-x86_64:<version>")
    runtimeOnly("com.oicana:oicana-linux-aarch64:<version>")
    runtimeOnly("com.oicana:oicana-macos-x86_64:<version>")
    runtimeOnly("com.oicana:oicana-macos-aarch64:<version>")
    runtimeOnly("com.oicana:oicana-windows-x86_64:<version>")
}
```

Declaring several natives is fine; the matching one is loaded at runtime. See the [latest release](https://central.sonatype.com/artifact/com.oicana/oicana) for the current version.

## Quick start

```java
import com.oicana.Template;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;

byte[] templateBytes = Files.readAllBytes(Path.of("invoice-0.1.0.zip"));

try (var template = new Template(templateBytes)) {
    byte[] pdf = template.export(
            Map.of("invoice", "{\"number\":\"2026-001\",\"customer\":\"Acme GmbH\",\"total\":\"€1,190.00\"}"),
            Map.of()
    );
    Files.write(Path.of("invoice.pdf"), pdf);
}
```

`export` returns the PDF bytes. `exportPng` and `exportSvg` produce the other formats, and `Template.exportOnce` renders a one-off template without registering it.

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

The [Oicana CLI](https://oicana.com/docs/cli/) does the packing, so a layout change ships as a new asset, not a code change.

## Running in a web service

Creating a `Template` compiles it once in development mode to warm up the Typst cache, so do it at startup, not per request. The instance is thread-safe and fits Spring Boot's singleton service scope; afterwards there is no file I/O on the hot path. `Template` implements `AutoCloseable` and frees its native resources on close.

## Why Oicana

- **Runs in your infrastructure**: PDFs are generated inside your own application. No data is sent to a third-party service.
- **Multi-platform**: the same template works in the browser, Node.js, C#, Java, Rust, Python, PHP, Go, Ruby, and Dart.
- **Powerful typesetting**: templates have all of Typst, including its package ecosystem.
- **Performant**: a warmed up template renders a PDF in single-digit milliseconds.
- **AI and version control ready**: templates are text files. They live next to your code, and AI can help write them.
- **No proprietary format**: templates are plain Typst projects. The Typst compiler is open source.

## Where to go next

- [Java / Spring Boot getting started guide](https://oicana.com/docs/getting-started/4-3-java/): from an empty project to a PDF endpoint
- [Open source Spring Boot example](https://github.com/oicana/oicana-example-java-spring-boot): blob inputs, error handling, and a preview endpoint
- [PDF generation on the JVM](https://oicana.com/pdf-generation/java/): the shorter overview
- [How Oicana compares](https://oicana.com/compare/): against headless browsers, PDF libraries, and hosted APIs

## Licensing

Oicana is source-available under the [PolyForm Noncommercial License 1.0.0](https://github.com/oicana/oicana/blob/main/LICENSE.md) and free for noncommercial use. Commercial use is free for 30 days; see [pricing](https://oicana.com/#pricing) for subscriptions, or write to `hello@oicana.com`.
