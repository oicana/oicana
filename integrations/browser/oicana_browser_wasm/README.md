# oicana_browser_wasm

The WebAssembly build of [Oicana](https://oicana.com) and its `wasm-bindgen` glue. It is not published on its own, but bundled into [`@oicana/browser`](https://www.npmjs.com/package/@oicana/browser), which wraps the raw exports in a documented, fully typed API.

## Development

Build the WASM into `../oicana-browser/wasm`:

```bash
npm run build:wasm
```

Run this in `integrations/browser/oicana-browser`, then `npm i && npm run build`. For faster iteration, run the `wasm-pack` command from the `build:wasm` script with `--no-opt` added to skip `wasm-opt`.

To try a local build in another project, `npm link` in `../oicana-browser`, then `npm link @oicana/browser` in that project.

## Licensing

Oicana is source-available under the [PolyForm Noncommercial License 1.0.0](https://github.com/oicana/oicana/blob/main/LICENSE.md) and free for noncommercial use. Commercial use is free for 30 days; see [pricing](https://oicana.com/#pricing) for subscriptions.
