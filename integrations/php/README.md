# Oicana PHP Integration

> Contributor documentation for building and publishing the PHP integration.
> Using Oicana in a PHP project? Start at [`oicana-php/README.md`](oicana-php/README.md)
> or the [PHP getting started guide](https://oicana.com/docs/getting-started/4-7-php/).

Oicana integration for PHP 8.3+.

## Structure

The PHP integration consists of three packages:

```
php/
├── oicana-php-native/      # Rust native extension (ext-php-rs)
├── oicana-php/             # PHP wrapper package
└── oicana-php-installer/   # Composer plugin for auto-installation
```

### oicana-php-native

Native Rust extension using [ext-php-rs](https://github.com/extphprs/ext-php-rs). Provides the low-level FFI bindings to the Oicana core.

### oicana-php

PHP wrapper providing an idiomatic API for template compilation and rendering.

### oicana-php-installer

Composer plugin that automatically downloads the appropriate native extension binary during installation.

## Development

### Prerequisites

- Rust 1.92+
- PHP 8.3+ with development headers
- Composer 2.0+

### Building the Native Extension

```bash
cd oicana-php-native
cargo build --release
```

The extension will be built to `../../target/release/liboicana_php_native.{so,dylib,dll}`.

### Installing Locally

After building, install the extension:

```bash
# Find your extension directory
php -i | grep extension_dir

# Copy the extension (.so on Linux, .dylib on macOS, .dll on Windows)
cp ../../target/release/liboicana_php_native.so /path/to/extensions/

# Add to php.ini
echo "extension=/path/to/extensions/liboicana_php_native.so" >> /path/to/php.ini
```

### Testing

#### PHP Wrapper Tests

The tests need the native extension loaded, so build it first and point PHP at the
local build.

```bash
cargo build --release -p oicana_php_native

cd oicana-php
composer install
php -d extension=../../../target/release/liboicana_php_native.so vendor/bin/pest
```

Append a path such as `tests/E2eTest.php` to run a single suite.

### Code Quality

```bash
cd oicana-php

# Static analysis
composer phpstan

# Code style check
composer cs-check

# Code style fix
composer cs-fix
```

## Publishing

### Native Extension

Native binaries are automatically built by the GitHub workflow (`.github/workflows/publish-integration-php.yml`) for all supported platforms:

- **PHP versions**: 8.3, 8.4, 8.5
- **Thread safety**: NTS, ZTS
- **Platforms**: Linux (x64, ARM64), macOS (x64, ARM64), Windows (x64)

Binaries are published to GitHub Releases.

### PHP Packages

The packages are published via a custom Composer repository hosted at [oicana/composer](https://github.com/oicana/composer).

**Release process:**

1. Tag a release: `git tag oicana_php-v0.1.0-rc.1`
2. Push: `git push --tags`
3. Workflow builds binaries and creates GitHub Release
4. Satis repository automatically updates within 24 hours (or can be triggered manually)

## Binary Naming Convention

Format: `oicana-php{VERSION}-{OS}-{ARCH}-{TS}.{EXT}`

Examples:
- `oicana-php8.3-linux-x64-nts.so`
- `oicana-php8.4-macos-arm64-zts.dylib`
- `oicana-php8.3-windows-x64-nts.dll`
