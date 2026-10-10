# oicana_uniffi

UniFFI bindings for Oicana. Exposes the API of `oicana_ffi_core` and builds as a Rust library, a shared library and a static library.

Generate bindings from the release build, which keeps the symbol table the generator reads. Strip the library before packaging it.

```sh
cargo build --release -p oicana_uniffi
cargo run -p uniffi-bindgen -- generate --library target/release/liboicana_uniffi.so --language <language> --out-dir <dir>
strip target/release/liboicana_uniffi.so
```

Generators must target the same UniFFI minor version as this crate.
