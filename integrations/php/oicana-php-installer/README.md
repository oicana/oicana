# Oicana PHP installer

> **This is an internal dependency of [Oicana](https://oicana.com).** Install the [`oicana/oicana`](https://github.com/oicana/oicana/tree/main/integrations/php/oicana-php) Composer package instead. It pulls this plugin in automatically.

Composer plugin that downloads and installs the Oicana native extension. On install it detects your platform (OS, architecture, PHP version, thread safety), fetches the matching binary from GitHub Releases, verifies it against the checksums bundled with this plugin, and prints how to enable it.

The plugin only installs the binary for its own version, so `oicana/oicana` and `oicana/installer` upgrade in lockstep.

## Supported platforms

| OS | Architecture | PHP 8.3 | PHP 8.4 | PHP 8.5 |
| -- | ------------ | ------- | ------- | ------- |
| Linux | x64 | ✓ | ✓ | ✓ |
| Linux | ARM64 | ✓ | ✓ | ✓ |
| macOS | x64 | ✓ | ✓ | ✓ |
| macOS | ARM64 | ✓ | ✓ | ✓ |
| Windows | x64 | ✓ | ✓ | ✓ |

Both NTS (non-thread-safe) and ZTS (thread-safe) builds are published. Requires Composer 2.0 or newer.

## If automatic installation fails

Download the binary for your platform from the `oicana_php` release on [the releases page](https://github.com/oicana/oicana/releases). Assets are named after PHP version, OS, architecture and thread safety, for example `oicana-php8.3-linux-x64-nts.so`. Add `extension=/full/path/to/oicana-php8.3-linux-x64-nts.so` to your `php.ini`. Building from source is covered in [`oicana-php-native`](https://github.com/oicana/oicana/tree/main/integrations/php/oicana-php-native).

## Licensing

Oicana is source-available under the [PolyForm Noncommercial License 1.0.0](https://github.com/oicana/oicana/blob/main/LICENSE.md) and free for noncommercial use. Commercial use is free for 30 days; see [pricing](https://oicana.com/#pricing) for subscriptions, or write to `hello@oicana.com`.
