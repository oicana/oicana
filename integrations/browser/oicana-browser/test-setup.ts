import { readFileSync } from 'node:fs';
import { beforeAll } from 'vitest';
import wasm from './wasm/oicana_browser_wasm.js';

beforeAll(async () => {
  await wasm(readFileSync('wasm/oicana_browser_wasm_bg.wasm'));
});
