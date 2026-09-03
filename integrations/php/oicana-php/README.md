# Oicana for PHP

*Generate PDFs in PHP without a headless browser.*

PHP teams generating PDFs usually pick between HTML-to-PDF libraries that struggle with page breaks and fonts, the abandoned wkhtmltopdf, or running a headless browser next to the application.

Oicana compiles PDFs in process through a native PHP extension instead. You design documents as [Typst](https://typst.app/) templates, load them once, and render them from JSON in single-digit milliseconds. No browser process, no per-document fees, no document data leaving your infrastructure.

> **Free for noncommercial use.** Commercial use is free for 30 days, then needs a [per-application subscription](https://oicana.com/#pricing) with unlimited seats.

## Installation

Oicana is distributed from its own Composer repository:

```bash
composer config repositories.oicana composer https://composer.oicana.com
composer config allow-plugins.oicana/installer true
composer require oicana/oicana
```

The installer downloads the native extension for your platform. Enable it with:

```bash
vendor/bin/oicana-env
```

That prints the `PHP_INI_SCAN_DIR` export for your platform; add it to your shell profile to make it permanent.

**Requirements:** PHP 8.3, 8.4, or 8.5 on Linux (x64/ARM64), macOS (x64/ARM64), or Windows (x64). Both NTS and ZTS builds are published.

## Quick start

```php
<?php

require 'vendor/autoload.php';

use Oicana\Template;

$template = new Template(file_get_contents('invoice-0.1.0.zip'));

try {
    $pdf = $template->export(
        jsonInputs: [
            'invoice' => [
                'number' => '2026-001',
                'customer' => 'Acme GmbH',
                'total' => '€1,190.00',
            ],
        ]
    );
    file_put_contents('invoice.pdf', $pdf);
} finally {
    $template->cleanup();
}
```

For a template you only render once, `Template::exportOnce()` handles registration and cleanup for you. It returns an `ExportOnceResult` with the document bytes and any compilation warnings:

```php
$result = Template::exportOnce(
    file_get_contents('invoice-0.1.0.zip'),
    jsonInputs: [
        'invoice' => [
            'number' => '2026-001',
            'customer' => 'Acme GmbH',
            'total' => '€1,190.00',
        ],
    ]
);

file_put_contents('invoice.pdf', $result->document);
```

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

## Usage

### Export formats

```php
use Oicana\ExportFormat;

$pdf = $template->export(exportFormat: ExportFormat::pdf());
$png = $template->export(exportFormat: ExportFormat::png(pixelsPerPt: 3.0));
$svg = $template->export(exportFormat: ExportFormat::svg());
```

`exportPdf()`, `exportPng()`, and `exportSvg()` are shorthands.

### Compilation modes

The mode can be set separately for creating a template (`new Template`) and exporting a document (`export`).

**Development mode** falls back to the `development` values from the manifest when an input is missing. **Production mode** requires every required input, so missing data fails loudly instead of silently rendering test values.

The defaults follow that split: `new Template()` compiles in development mode, falling back to the `development` and `default` values in the manifest, and `export()` uses production mode. Creating a template therefore needs no inputs of its own, as long as every required input has a fallback value.

```php
use Oicana\CompilationMode;

$data = ['invoice' => [
    'number' => '2026-001',
    'customer' => 'Acme GmbH',
    'total' => '€1,190.00',
]];

// The defaults: development for creation, production for export
$template = new Template($bytes);
$pdf = $template->export(jsonInputs: $data);

// Override either one
$template = new Template($bytes, jsonInputs: $data, mode: CompilationMode::Production);
$pdf = $template->export(jsonInputs: $data, mode: CompilationMode::Development);
```

### Inputs

JSON inputs take arrays (encoded for you) or pre-encoded JSON strings:

```php
$pdf = $template->export(
    jsonInputs: [
        'user' => ['name' => 'Alice', 'email' => 'alice@example.com'],
        'items' => [['id' => 1, 'name' => 'Item 1']],
        'raw' => '{"already": "encoded"}',
    ]
);
```

Blob inputs carry binary data such as images or fonts, with optional metadata:

```php
use Oicana\Inputs\BlobInput;

$logo = new BlobInput(file_get_contents('logo.png'), ['type' => 'image/png']);

$pdf = $template->export(blobInputs: ['logo' => $logo]);
```

### Template introspection

```php
$manifest = $template->manifest();       // package section and Oicana config, including inputs
$source = $template->source('main.typ'); // Typst source of a packed file
$file = $template->file('assets/logo.png'); // binary file from the archive
```

## Why Oicana

- **Runs in your infrastructure**: PDFs are generated inside your own application. No data is sent to a third-party service.
- **Multi-platform**: the same template works in the browser, Node.js, C#, Java, Rust, Python, and PHP.
- **Powerful layouting**: templates have all of Typst, including its package ecosystem.
- **Performant**: a warmed up template renders a PDF in single-digit milliseconds.
- **AI and version control ready**: templates are text files. They live next to your code, and AI can help write them.
- **No proprietary format**: templates are plain Typst projects. The Typst compiler is open source.

## Where to go next

- [PHP / Slim getting started guide](https://oicana.com/docs/getting-started/4-7-php/): from an empty project to a PDF endpoint
- [Open source Slim example](https://github.com/oicana/oicana-example-php-slim): blob inputs, error handling, and a preview endpoint
- [PDF generation in PHP](https://oicana.com/pdf-generation/php/): the shorter overview
- [How Oicana compares](https://oicana.com/compare/): against headless browsers, PDF libraries, and hosted APIs

## Licensing

Oicana is source-available under the [PolyForm Noncommercial License 1.0.0](https://github.com/oicana/oicana/blob/main/LICENSE.md) and free for noncommercial use. Commercial use is free for 30 days; see [pricing](https://oicana.com/#pricing) for subscriptions, or write to `hello@oicana.com`.
