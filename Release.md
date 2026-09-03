# Releasing Oicana

1. Bump general crate version in `cargo.toml`
  * Also bump the versions in the workspace dependencies
2. Bump CLI version
3. Integrations
  * bump browser
    * wasm in integrations/browser/oicana_browser_wasm/Cargo.toml
    * wrapper und wasm dep in integrations/browser/oicana-browser/package.json
  * bump C# integration in integrations/csharp/Oicana/Oicana.csproj and integrations/csharp/oicana_csharp/Cargo.toml
  * bump Node.Js integration versions 
    * in integrations/node/oicana-node-native/Cargo.toml and integrations/node/oicana-node-native/package.json
      * run `yarn build` and `yarn format`
    * wrapper integrations/node/oicana-node/package.json
  * Python:
    * native: integrations/python/oicana-python-native/pyproject.toml and integrations/python/oicana-python-native/Cargo.toml
    * wrapper: integrations/python/oicana-python/pyproject.toml
    * integrations/python/oicana-python/src/oicana/__init__.py
    * run `uv sync` in integrations/python/oicana-python
  * PHP:
    * integrations/php/oicana-php-native/Cargo.toml
    * integrations/php/oicana-php/composer.json
    * integrations/php/oicana-php-installer/composer.json
  * Java:
    * integrations/java/oicana-java-native/Cargo.toml
    * integrations/java/build.gradle.kts (version in `subprojects` block)
  * Typst package
    * Bump version in integrations/typst/typst.toml
    * Bump code version and version in docs in integrations/typst/src/lib.typ
4. Run `cargo publish --workspace` to publish the rust integration and CLI to crates.io
5.
  * tag CLI version `oicana_cli-v*` => will trigger CD pipeline
  * tag crates version `oicana_rust-v*`
  * tag C# version `oicana_csharp-v*` => will trigger CD pipeline
  * tag browser `oicana_browser-v*` => will trigger CD pipeline
  * tag node `oicana_node-v*` => will trigger CD pipeline
  * tag python `oicana_python-v*` => will trigger CD pipeline
  * tag PHP `oicana_php-v*` => will trigger CD pipeline
  * tag Java `oicana_java-v*` => will trigger CD pipeline

## CLI

The CLI is distributed through cargo-dist. Push a tag in the form of `oicana_cli-v0.0.0-rc.1` to trigger the workflow.
It will build and package the CLI for all platforms currently set up.

The release pipeline is configured to run with latest stable rust.

Remember to publish the CLI to crates.io as well for proper `cargo binstall` support (see "Crates" section below).

### Updating dist

Run `dist init` to update the config and workflow.

## C#

The github workflow `.github/workflows/publish_csharp.yml` can be manually triggered or by pushign a tag in the form of `oicana_csharp-v[0-9]+.[0-9]+.[0-9]+*`.
It will build the native libraries for Linux, MacOS and Windows. Then all native libraries are
included in the C# package and bundled into a `.nupkg` which will be archived by the workflow.

The pipeline pushes the new version to nuget.

## WASM

The WASM integration is published in two packages, both from `.github/workflows/publish-integration-browser.yml`.
The workflow runs on a tag `oicana_browser-v0.0.0-rc.1` or manual trigger, and publishes `@oicana/browser-wasm` first,
then `@oicana/browser`.

1. `oicana_browser_wasm`
  - Bump version in `integrations/browser/oicana_browser_wasm/Cargo.toml`
2. `oicana-browser`
  - Bump `@oicana/browser-wasm` dependency
  - Bump version
  - Run `npm i` to update lock file

### Pipeline authentication

This applies to both npm workflows, `publish-integration-browser.yml` and `publish-integration-node.yml`.

Publishing uses npm trusted publishing (OIDC).

## Node


The Node integration is published in two steps, first `@oicana/node-native` then `@oicana/node`.

Bump the version of `@oicana/node-native` in `integrations/node/oicana-node-native/Cargo.toml` and `integrations/node/oicana-node-native/package.json`
then rebuild the bindings with `yarn build` and format them via `yarn format`.

The CD pipeline runs on a tag `oicana_node-v0.0.0-rc.1` or manual trigger. It publishes `@oicana/node-native` with its
platform packages, then the wrapper `@oicana/node`. Each package is only published if that version doesn't exist on the
index yet. Publishing uses npm trusted publishing (OIDC).

## PHP

Push a tag `oicana_php-v0.0.0-rc.1` or manually trigger .github/workflows/publish-integration-php.yml with a version input.

The CD pipeline validates version consistency across all 3 version sources, builds native extensions for PHP 8.3/8.4/8.5 (NTS + ZTS) on Linux, macOS, and Windows, creates a GitHub release with binaries, and triggers a Composer registry rebuild.

## Java

Push a tag `oicana_java-v0.0.0-rc.1` or manually trigger .github/workflows/publish-integration-java.yml with a version input.

The CD pipeline validates version consistency, builds JNI native libraries for 5 platforms, and publishes 6 artifacts to Maven Central via the Sonatype Central Portal:
- `com.oicana:oicana` (API JAR)
- `com.oicana:oicana-{platform}` (native JARs per platform)

## Crates

All rust crates excluding the native wrappers are published to crates.io. This includes the CLI project `oicana_cli`.

Run `cargo publish --workspace --dry-run` to try publishing all crates.

## Updating the third party license file

This project uses ORT
* `~/ort-path/bin/ort --info analyze -f JSON -i . -o ./ort`
* `~/ort-path/bin/ort --info report -f PlainTextTemplate -i ./ort/analyzer-result.json -o ./ort`
