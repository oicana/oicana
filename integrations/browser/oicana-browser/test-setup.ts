import { readFileSync } from 'node:fs';
import wasm from '@oicana/browser-wasm';
import { beforeAll } from 'vitest';

beforeAll(async () => {
  const wasmModule = readFileSync(
    'node_modules/@oicana/browser-wasm/oicana_browser_wasm_bg.wasm',
  );
  await wasm(wasmModule);
});
