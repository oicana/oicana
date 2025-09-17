import init from '@oicana/browser-wasm';

export * from './inputs';
export * from './Template';

const initialized: Set<string> = new Set();

/**
 * Initializes the WASM module from the given URL
 * @param wasmPath URL from which to load the WASM module
 */
export async function initialize(wasmPath: string): Promise<void> {
  if (initialized.has(wasmPath)) return;
  await init(wasmPath);
  initialized.add(wasmPath);
}
