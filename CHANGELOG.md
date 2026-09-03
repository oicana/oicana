# Changelog

## Upcoming

- registering fonts by path accepts a directory and adds every font file in its tree, like the CLI's `--font-path` already did
- `Template.inputs` is now `Template.manifest` in every integration and returns a typed manifest
  - the Typst package section and the whole Oicana configuration, including the input definitions

### CLI
- `pack` no longer writes directory entries for directories that end up with no packed content, for example a directory whose files are all excluded

### Java
- A JVM that has not granted native access now fails with an `OicanaException` naming the required `--enable-native-access` flag
- All jars got valid module names

### Node.js
- `engines` declares the actual minimum of `^20.19.0 || >=22.12.0`; the package is ESM-only, so CommonJS callers such as NestJS need a Node version with `require(esm)`
- Publish a `linux-x64-musl` build
- `BlobWithMetadata` is now `BlobInput`, its fields `bytes` and `meta` are now `data` and `metadata`

### Browser
- `BlobWithMetadata` is now `BlobInput`, its fields `bytes` and `meta` are now `data` and `metadata`

### C#
- `BlobInput.Meta` is now `BlobInput.Metadata`

### Rust
- `Template::init_with_limits` and `Template::init_with_fonts_and_limits` take custom `ZipLimits`

## v0.8.0

- an unsupported `manifest_version` gets rejected by the CLI and by every integration
- all integrations now validate pixels per pt for png exports to be finite and positive
- Unify zip limit validation in the integrations

### CLI
- `test` exports every test case to PDF with the standards configured in the template manifest
  - tests and test collections can opt out with `pdf = false`
- `snapshot` can be configured for a test collection
- `validate` checks that `default` and `development` value files exist
- `validate` warns about required inputs that have no `default` or `development` value
- `validate --deny-warnings` fails the validation if any warnings are reported
- Nicer error messages

### Rust
- `TemplateInitializationError` and `CompileError` forward their source errors transparently instead of re-printing them

### Browser
- stop mutating user's blob with metadata objects
- remove exports for unused/unusable `NOT_REGISTERED` and `setValidateInputs`

### Node
- remove exports for unused/unusable `NOT_REGISTERED` and `setValidateInputs`

### C#
- `ZipLimits` rejects negative bounds instead of silently falling back to the default

### Java
- `ZipLimits` rejects negative bounds instead of silently falling back to the default

## v0.7.0

- Hosts can provide fonts to templates on top of the ones a template packs itself
  - fonts are registered per process and shared by every template, so a large font costs memory once instead of once per template
  - fonts registered by path do not retain their data until it is used
- Templates can declare the font families they expect from their host with `[tool.oicana.fonts] require`
- The fonts embedded in Typst are parsed once per process instead of for every template, which speeds up registering templates and one-off exports
- Update Typst from 0.15.0 to 0.15.1

### CLI
- `compile`, `watch` and `test` accept `--font-file <FILE>` and `--font-path <DIR>` (also read from `OICANA_FONT_PATHS`) to mimic a host that registers fonts
- `validate` and `pack` report why a `typst.toml` could not be read instead of "No valid Oicana template found"

### Internal
- `assets/fonts/oicana-test-font.ttf` (family `Oicana Test`) backs the host-font tests of every integration, so they no longer depend on the fonts installed on the machine running them

### Rust
- `Template::init_with_fonts` / `Template::from_with_fonts` and `OicanaWorld::new_with_fonts`

### Python
- Expose `register_fonts`, `register_font_paths`, `registered_fonts` and `clear_fonts`

### Node.js
- Expose `registerFonts`, `registerFontPaths`, `registeredFonts` and `clearFonts`

### Browser
- Expose `registerFonts`, `registeredFonts` and `clearFonts`; there is no path-based variant, since there is no filesystem to read fonts from

### PHP
- Expose `Configuration::registerFonts`, `registerFontPaths`, `registeredFonts` and `clearFonts`
- Fix: `new BlobInput($data, [])` no longer fails with "invalid type: sequence, expected a map"

### Java
- Expose `Configuration.registerFonts`, `registerFontPaths`, `registeredFonts` and `clearFonts`

### C#
- Expose `Configuration.RegisterFonts`, `RegisterFontPaths`, `RegisteredFonts` and `ClearFonts`
- Move `OicanaException` to root namespace


## v0.6.0

- Performance improvement: more finegrained internal locking for exports to unlock more parallelization
- Update the time for compilations when updating inputs

### PHP
- Performance improvements
- Expose exportOnce for exportign a template without caching it or the document
- Allow setting the limits when reading a packed template
- Expose setter for the diagnostic coloring

### C#
- Breaking: `Template` constructors no longer accept a custom template id
- Resolved possible memory leaks in error cases and if users forget to dispose a document stream
- ExportOnce now returns compilation warnings as well as the export result
- Allow setting the limits when reading a packed template

### Java
- Fixed JSON encoding
- Catch panics in `configureAutomaticCacheEviction` and `evictCache`
- Expose exportOnce for exportign a template without caching it or the document
- Allow setting the limits when reading a packed template
- Expose setter for the diagnostic coloring

### Node.js
- Method for async template registration
- Complete methods on `Template` with `inputs`, `source`, and `file`
- Expose exportOnce for exportign a template without caching it or the document
- Allow setting the limits when reading a packed template

### Browser
- Expose exportOnce for exportign a template without caching it or the document
- Allow setting the limits when reading a packed template

### Python
- Expose exportOnce for exportign a template without caching it or the document
- Allow setting the limits when reading a packed template
- Expose setter for the diagnostic coloring


## v0.5.0

- Fix potential ABBA deadlock when doing template compilation in parallel to document exports
- Performance improvement: more finegrained internal locking to unlock more parallelization
- Bumped some dependencies to resolve security advisories
- Limit packed template archives to protect against zip bombs
  - Default limits: 10,000 entries and 512 MiB of decompressed content 
- Limit pixels on png exports to keep the memory usage of the export in an acceptable range
  - Default limit: 256 million pixels or ~1Gb of memory
  - corresponds to roughly 14 A4 pages at 300 DPI or one A4 page at 800 DPI
- Fix: Recover from poisoned file map locks in packed templates

### CLI
- `pack` warns if the packed template exceeds the default archive limits

### Python integration
- release GIL where possible to allow parallelization

### PHP integration
- prevent panics across FFI border

### C# integration
- prevent panics across FFI border
- Fix non-ASCII string handling on Windows
- Fixed memory leak in input handing

### Node integration
- prevent panics across FFI border
- Offer async methods to offload compilation and document export to the libuv thread pool


## v0.4.0
- Update to Typst 0.15
  - It's now possible to export a combination of PDF standards
- Patched used Typst to support custom XMP metadata
  - This enables use-cases like e-invoices with factur-x/ZUGFeRD standards

### Python integration
- Lower the minimum supported Python version from 3.11 to 3.9
- Publish `musllinux` wheels (x86_64 and aarch64)


## v0.3.0
- previous `compile` methods in all integrations are now called `export`
- add new `compile` methods that return compiled documents, which can be
  - exported to any format
  - exported page by page
- templates and compiled documents offer `exportPdf`/`exportPng`/`exportSvg` convenience
  methods in all integrations
- compiled documents expose the compilation `warnings` in all integrations
- PDF tagging can be toggled via `tagged` in `[tool.oicana.export.pdf]` (defaults to `true`)


## v0.2.0
- fixed some edge-case panics is the central crates

### Rust integration v0.2.0
- re-export more Typst types

### CLI v0.2.0
- Compress images before writing them to disc (snapshots and comparison files)

### Typst package v0.2.0
- fix image helper for optional blob inputs

### Browser integration v0.2.0
- Expose getter for compilation warnings
- Move standard logs to trace
- Use explicit extensions in imports

### C# integration v0.2.0
- Expose getter for compilation warnings

### Java integration v0.2.0
- Expose getter for compilation warnings

### Node.js integration v0.2.0
- Expose getter for compilation warnings

### PHP integration v0.2.0
- Expose getter for compilation warnings

### Python integration v0.2.0
- Expose getter for compilation warnings




## Alpha

### CLI v0.1.0-alpha.17
- set document date on template scaffold

### CLI v0.1.0-alpha.16
- template now always exclude output and test directories
- validate command will ensure the given pdf standards for export are compatible
- `new` command

### Rust integration v0.1.0-alpha.12
<changes limited to internal crates for the CLI>

### C# integration v0.1.0-alpha.12
- `Oicana.Template` namespace was removed; all content moved to the root namespace

### Rust integration v0.1.0-alpha.11
- re-export all required oicana crates and Typst types
- more granular features in the integration and internal crates

### CLI v0.1.0-alpha.15
- Update to rust integration v0.1.0-alpha.11
- Show version of the bundled Typst compiler in version output

### Browser integration v0.1.0-alpha.8
- Use non deprecated parameter for WASM init call

### C# integration v0.1.0-alpha.11
- rename `ExportOptions` -> `ExportFormat` for consistency

### PHP integration v0.1.0-alpha.5
- Change pixels per pt default for png exports from 2 to 1 for consistency

### CLI v0.1.0-alpha.14 (WIP in 12/13)
- Update command when installing CLI via scripts

### CLI v0.1.0-alpha.11
### Rust integration v0.1.0-alpha.10
- watch mode for tests

### CLI v0.1.0-alpha.10
### Rust integration v0.1.0-alpha.9
### Typst integration v0.1.1
- Make inputs required by default with better error handling
- Support watch command in CLI
- Don't copy template dependencies on disk
- General CLI output improvements

### CLI v0.1.0-alpha.9
### Browser integration v0.1.0-alpha.7
### C# integration v0.1.0-alpha.10
### Node.js integration v0.1.0-alpha.8
### Python integration v0.1.0-alpha.5
### PHP integration v0.1.0-alpha.4
### Java integration v0.1.0-alpha.3
### Rust integration v0.1.0-alpha.8
- Add JSON input validation for inputs with schemas

### Browser integration v0.1.0-alpha.6
- return Uint8Array<ArrayBuffer> from compile methods
- Implement Disposable for automatic cleanup with `using`

### Node.js integration v0.1.0-alpha.7
- Implement Disposable for automatic cleanup with `using`
- more error handling
- expose cache eviction methods

### PHP integration v0.1.0-alpha.3
- expose cache eviction methods

### Python integration v0.1.0-alpha.4
- expose cache eviction methods
- rename `export_format` parameter to `export`

### Rust integration v0.1.0-alpha.7
- follow sym links when packing templates
- remove some possible panics

### CLI v0.1.0-alpha.8
- print packaged templates
- follow sym links when packing templates
- remove some possible panics

### C# integration v0.1.0-alpha.9
- Expose cache eviction on `Configuration` and `Template`

### Java integration v0.1.0-alpha.2
- improve overloads for Template#Compile

### C# integration v0.1.0-alpha.8
- Take inputs as dictionary
- Remove possible panics in error handling


### Browser integration v0.1.0-alpha.4
### Node.js integration v0.1.0-alpha.5
- Remove log statements

### C# integration v0.1.0-alpha.6
### Browser integration v0.1.0-alpha.3
### Node.js integration v0.1.0-alpha.4
- Unify function parameters
- Fix default compilation mode

### Crates v0.1.0-alpha.5
### CLI v0.1.0-alpha.6
### C# integration v0.1.0-alpha.5
### Browser integration v0.1.0-alpha.2
### Node.js integration v0.1.0-alpha.3
### Rust integration v0.1.0-alpha.5
- Fix template paths on Windows
- Update to Typst 0.14

### Browser v0.1.0-alpha.2
- improved Browser compatibility (e.g. works on newer Firefox Android now) 

### C# integration v0.1.0-alpha.4
- DO NOT USE; broken on Windows

### Crates v0.1.0-alpha.4 and CLI v0.1.0-alpha.5
- fix fuzzing tests from paths other than template root ((#28)[https://github.com/oicana/oicana/pull/20])

### Crates v0.1.0-alpha.3 and CLI v0.1.0-alpha.4
- mostly changes for CLI - v0.1.0-alpha.3

### CLI - v0.1.0-alpha.3
- fix CLI sometimes packaging test files ((#20)[https://github.com/oicana/oicana/pull/20])
- tests can be without a snapshot file ((#16)[https://github.com/oicana/oicana/pull/16])
- tests can fuzz json inputs with a json schema ((#16)[https://github.com/oicana/oicana/pull/16])
- tests will fail for missing snapshots ((#17)[https://github.com/oicana/oicana/pull/17])
- new options for test command `--update`/`-u` will overwrite/create snapshot files ((#17)[https://github.com/oicana/oicana/pull/17])
