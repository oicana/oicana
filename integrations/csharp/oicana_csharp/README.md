# oicana_csharp

> **This is an internal build artifact of [Oicana](https://oicana.com).** Install the [`Oicana`](https://www.nuget.org/packages/Oicana) NuGet package instead. It ships this native library and wraps it in the documented API.

FFI bindings consumed by the C# wrapper package in [`../Oicana.Core`](https://github.com/oicana/oicana/tree/main/integrations/csharp/Oicana.Core). The exported functions take and return raw handles, with no stability guarantees between releases.

## Development

```sh
cargo run --example csharp_bindings -p oicana_csharp && cargo build --release -p oicana_csharp
```

This generates the C# interop class into the wrapper project and builds the dynamic library. In Debug builds the wrapper copies the library from `target/` into its own `bin/` directory.

## .NET support

The wrapper package targets .NET 8.0 and newer. If you need an older version, write to `hello@oicana.com`.

## Licensing

Oicana is source-available under the [PolyForm Noncommercial License 1.0.0](https://github.com/oicana/oicana/blob/main/LICENSE.md) and free for noncommercial use. Commercial use is free for 30 days; see [pricing](https://oicana.com/#pricing) for subscriptions.
