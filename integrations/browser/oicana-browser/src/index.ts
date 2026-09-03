import init, {
  configure_diagnostic_color,
  registered_fonts,
} from '@oicana/browser-wasm';

export {
  clear_fonts as clearFonts,
  configure_automatic_cache_eviction as configureAutomaticCacheEviction,
  evict_cache as evictCache,
  register_fonts as registerFonts,
  set_log_level as setLogLevel,
} from '@oicana/browser-wasm';
export * from './BlobInput.js';
export * from './CompilationMode.js';
export * from './CompiledDocument.js';
export * from './ExportFormat.js';
export * from './ExportOnceResult.js';
export * from './PageRange.js';
export * from './Template.js';
export * from './TemplateManifest.js';
export * from './ZipLimits.js';

/** Color mode for compilation diagnostics. */
export type DiagnosticColor = 'none' | 'ansi';

/**
 * Configure the coloring of compilation diagnostics like warnings and errors.
 */
export function configureDiagnosticColor(color: DiagnosticColor): void {
  configure_diagnostic_color(color === 'ansi');
}

/** A font face made available to templates by the host. */
export interface RegisteredFont {
  /** The family name, as used in Typst's `text(font: ...)`. */
  family: string;
  /**
   * The file the face was read from. Always absent in the browser, where fonts
   * can only be registered from memory.
   */
  path?: string;
}

/**
 * All font faces currently registered with `registerFonts`.
 */
export function registeredFonts(): RegisteredFont[] {
  return registered_fonts() as RegisteredFont[];
}

const initialized: Set<string> = new Set();

/**
 * Initializes the WASM module from the given URL
 * @param wasmPath URL from which to load the WASM module
 */
export async function initialize(wasmPath: string): Promise<void> {
  if (initialized.has(wasmPath)) return;
  await init({ module_or_path: wasmPath });
  initialized.add(wasmPath);
}
