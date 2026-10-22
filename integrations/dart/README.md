# Oicana for Dart and Flutter

*Generate typeset PDFs in Dart and Flutter.*

Flutter apps generating PDFs usually lay out every page in code with the `pdf` package, or send their data to a server that renders the document. The first buries the document design in widget code, the second needs a backend and an internet connection.

Oicana compiles PDFs in process instead. You design documents in [Typst](https://typst.app/), and render them from JSON on the device or on your server. No per-document fees, no document data leaving the device or your infrastructure.

> **Free for noncommercial use.** Commercial use is free for 30 days, then needs a [per-application subscription](https://oicana.com/#pricing) with unlimited seats.

## Installation

```bash
flutter pub add oicana   # or: dart pub add oicana
```

Requires Dart 3.11 or Flutter 3.41 or newer. The package works in Flutter apps and in plain Dart programs, like servers and command line tools, on:

- Android (arm64, arm, x64)
- iOS (devices and simulators)
- Linux (x64 and arm64, glibc 2.28 or newer)
- macOS (x64 and arm64)
- Windows (x64 and arm64)
- the web, via WebAssembly

When you build or run an app for a native target, a build hook downloads the native Oicana library for exactly that target and bundles it. Web builds of Flutter apps bundle the WebAssembly build instead.

> Deploy Dart programs, like servers, with `dart build cli`, which puts the native library next to the executable. `dart compile exe` does not support packages with build hooks like `oicana`.

## Quick start

Oicana templates are Typst projects packed with the [Oicana CLI](https://oicana.com/docs/cli/). This example uses the certificate template from [oicana-example-templates](https://github.com/oicana/oicana-example-templates). Download the packed `certificate-0.1.0.zip` from its releases, or pack it yourself with `oicana pack templates/certificate`. The [getting started guide](https://oicana.com/docs/getting-started/1-setup/) walks you through writing your own.

```dart
import 'dart:io';

import 'package:oicana/oicana.dart';

Future<void> main() async {
  final template = await Template.load(
    File('certificate-0.1.0.zip').readAsBytesSync(),
  );

  final pdf = await template.exportPdf(
    jsonInputs: {
      'certificate': {'name': 'Ada Lovelace'},
    },
  );
  File('certificate.pdf').writeAsBytesSync(pdf);

  template.dispose();
}
```

JSON inputs take a `String` holding a JSON document, or any value `jsonEncode` accepts. Blob inputs take `BlobInput` values with optional metadata.

## In Flutter apps

You can add packed Oicana templates to the assets of your app and load them with `rootBundle`:

```dart
final bytes = await rootBundle.load('assets/certificate-0.1.0.zip');
final template = await Template.load(bytes.buffer.asUint8List());
```

Loading a template compiles it once to warm up the Typst cache, so load it once, not for every document. Methods without the `Sync` suffix run on a separate isolate on native targets and keep your UI responsive. Templates and compiled documents are freed by the garbage collector, or right away with `dispose`.

## On the web

On the web, Oicana runs as WebAssembly. Call `await initOicana()` before the first synchronous call; the asynchronous methods load Oicana on their own.

Dart web apps without Flutter have no asset bundling, so they need some setup. Copy `oicana_browser_wasm.js` and `oicana_browser_wasm_bg.wasm` from the `web` directory of this package next to the files your server delivers, and pass the URL of the JavaScript file:

```dart
await initOicana(wasmModule: Uri.parse('/oicana/oicana_browser_wasm.js'));
```

The WebAssembly build is about 40 MB, so let your server compress it. By default it runs on the UI thread, so exports block the page while they run. Errors on the web are `OicanaException` values without the specific subclass.

## Builds without internet access

Download the libraries from the [GitHub release](https://github.com/oicana/oicana/releases) of `oicana_uniffi` matching your version of this package, decompress them into one directory per Rust target, like `aarch64-linux-android/liboicana_uniffi.so`, and point the build hook at that directory in the `pubspec.yaml` of your app:

```yaml
hooks:
  user_defines:
    oicana:
      native_library_dir: native/oicana
```

## Why Oicana

- **Runs in your infrastructure**: PDFs are generated inside your own application. No data is sent to a third-party service.
- **Multi-platform**: the same template works in the browser, Node.js, C#, Java, Rust, Python, PHP, Go, Ruby, and Dart.
- **Powerful typesetting**: templates have all of Typst, including its package ecosystem.
- **Performant**: a warmed up template renders a PDF in single-digit milliseconds.
- **AI and version control ready**: templates are text files. They live next to your code, and AI can help write them.
- **No proprietary format**: templates are plain Typst projects. The Typst compiler is open source.

## Where to go next

- [Dart / Flutter getting started guide](https://oicana.com/docs/getting-started/4-10-dart/): from an empty project to a PDF in your app
- [How Oicana compares](https://oicana.com/compare/): against headless browsers, PDF libraries, and hosted APIs

## Licensing

Oicana is source-available under the [PolyForm Noncommercial License 1.0.0](https://github.com/oicana/oicana/blob/main/LICENSE.md) and free for noncommercial use. Commercial use is free for 30 days; see [pricing](https://oicana.com/#pricing) for subscriptions, or write to `hello@oicana.com`.
