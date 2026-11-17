# Oicana PHP native extension

> **This is an internal build artifact of [Oicana](https://oicana.com).** Install the [`oicana/oicana`](https://github.com/oicana/oicana/tree/main/integrations/php/oicana-php) Composer package instead. It downloads this extension for your platform and wraps it in the documented API.

Native PHP extension built with [ext-php-rs](https://github.com/extphprs/ext-php-rs). The functions here take and return raw handles, with no stability guarantees between releases.

## Building

Requires Rust 1.92+ and PHP 8.3, 8.4, or 8.5 with development headers.

```bash
cargo build --release
```

The extension lands in `target/release/` as `liboicana_php_native.so` (Linux), `liboicana_php_native.dylib` (macOS), or `oicana_php_native.dll` (Windows).

## Installing locally

```bash
php -i | grep extension_dir                       # find your extension directory
cp target/release/liboicana_php_native.so "$dir"  # copy it there
echo "extension=oicana_php_native.so" >> php.ini  # and load it
```

Or load it for a single command with `php -d extension=/path/to/liboicana_php_native.so`.

## Licensing

Oicana is source-available under the [PolyForm Noncommercial License 1.0.0](https://github.com/oicana/oicana/blob/main/LICENSE.md) and free for noncommercial use. Commercial use is free for 30 days; see [pricing](https://oicana.com/#pricing) for subscriptions.
