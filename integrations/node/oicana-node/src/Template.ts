import { randomUUID } from 'node:crypto';
import {
  type BlobWithMetadata as BlobWithMetadataNative,
  compileTemplate,
  compileTemplateAsync,
  exportDocument,
  exportDocumentAsync,
  exportTemplateOnce,
  exportTemplateOnceAsync,
  getFile,
  getSource,
  getWarnings,
  CompilationMode as NativeCompilationMode,
  manifest as nativeManifest,
  registerTemplate,
  registerTemplateAsync,
  removeDocument,
  removeWorld,
  setValidateInputs,
} from '@oicana/node-native';
import type { BlobInput } from './BlobInput.js';
import { CompilationMode } from './CompilationMode.js';
import { CompiledDocument } from './CompiledDocument.js';
import { type ExportFormat, Pdf, Png, Svg } from './ExportFormat.js';
import type { ExportOnceResult } from './ExportOnceResult.js';
import { type PageRange, serializePageRange } from './PageRange.js';
import type { TemplateManifest } from './TemplateManifest.js';
import type { ZipLimits } from './ZipLimits.js';

/**
 * Marks a constructor call from {@link Template.create}, where the template
 * has already been registered on a background thread.
 */
const alreadyRegistered = Symbol('oicana-already-registered');

interface CompletedRegistration {
  readonly token: typeof alreadyRegistered;
  readonly templateId: string;
  readonly warnings: string | undefined;
}

/**
 * The token symbol is module-private, so only {@link Template.create} can
 * produce a value satisfying this check.
 */
function isCompletedRegistration(
  value: Uint8Array | CompletedRegistration,
): value is CompletedRegistration {
  return (
    typeof value === 'object' &&
    value !== null &&
    (value as CompletedRegistration).token === alreadyRegistered
  );
}

/**
 * A template
 *
 * The zip file is loaded during the instance creation and cached afterward.
 */
export class Template implements Disposable {
  private readonly template: string;
  private lastWarnings: string | undefined;

  /**
   * Register a template with the given template file
   * @param template - the packed Oicana template file
   */
  public constructor(template: Uint8Array);

  /**
   * Register a template with the given template file and inputs
   * @param template - the packed Oicana template file
   * @param jsonInputs - for the initial compilation to warm up the cache
   * @param blobInputs - for the initial compilation to warm up the cache
   */
  public constructor(
    template: Uint8Array,
    jsonInputs: Map<string, string>,
    blobInputs: Map<string, BlobInput>,
  );

  /**
   * Register a template with the given template file and inputs
   * @param template - the packed Oicana template file
   * @param jsonInputs  -for the initial compilation to warm up the cache (defaults to empty map)
   * @param blobInputs - for the initial compilation to warm up the cache (defaults to empty map)
   * @param compilationOptions - for the initial compilation to warm up the cache (defaults to Development)
   * @param limits - for reading the template zip (defaults apply when omitted)
   */
  public constructor(
    template: Uint8Array,
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
    limits?: ZipLimits,
  );

  public constructor(
    template: Uint8Array | CompletedRegistration,
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
    limits?: ZipLimits,
  ) {
    if (isCompletedRegistration(template)) {
      this.template = template.templateId;
      this.lastWarnings = template.warnings;
      return;
    }

    if (!(template instanceof Uint8Array)) {
      throw new TypeError(
        'template must be a Uint8Array containing the packed template file',
      );
    }

    this.template = randomUUID();

    const documentId = registerTemplate(
      this.template,
      template,
      Object.fromEntries(jsonInputs ?? new Map<string, string>()),
      Template.convertBlobInputs(blobInputs ?? new Map<string, BlobInput>()),
      Template.mapCompilationMode(
        compilationOptions ?? CompilationMode.Development,
      ),
      limits,
    );
    this.lastWarnings = getWarnings(documentId) ?? undefined;
    removeDocument(documentId);
  }

  /**
   * Register a template on a background thread and resolve to the prepared
   * {@link Template}.
   *
   * Unlike the constructor, this does not block the Node.js event loop while
   * the template is read and its warm-up compilation runs.
   * @param template - the packed Oicana template file
   * @param jsonInputs - for the initial compilation to warm up the cache (defaults to empty map)
   * @param blobInputs - for the initial compilation to warm up the cache (defaults to empty map)
   * @param compilationOptions - for the initial compilation to warm up the cache (defaults to Development)
   * @param limits - for reading the template zip (defaults apply when omitted)
   */
  public static async create(
    template: Uint8Array,
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
    limits?: ZipLimits,
  ): Promise<Template> {
    const templateId = randomUUID();

    const documentId = await registerTemplateAsync(
      templateId,
      template,
      Object.fromEntries(jsonInputs ?? new Map<string, string>()),
      Template.convertBlobInputs(blobInputs ?? new Map<string, BlobInput>()),
      Template.mapCompilationMode(
        compilationOptions ?? CompilationMode.Development,
      ),
      limits,
    );
    const warnings = getWarnings(documentId) ?? undefined;
    removeDocument(documentId);

    const registration: CompletedRegistration = {
      token: alreadyRegistered,
      templateId,
      warnings,
    };
    return new Template(registration as never);
  }

  /**
   * Compile and export a template in a single native call, without caching.
   *
   * Nothing is registered and no warm-up compilation runs, so this is the
   * fastest way to render a template exactly once. For repeated exports of the
   * same template, create a {@link Template} instance instead.
   * @param template - the packed Oicana template file
   * @param jsonInputs - JSON inputs for the template (defaults to empty map)
   * @param blobInputs - Blob inputs for the template (defaults to empty map)
   * @param exportOptions - Export format specification (defaults to PDF)
   * @param compilationOptions - Compilation mode (defaults to Production)
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   * @param limits - limits for reading the template zip (defaults apply when omitted)
   */
  public static exportOnce(
    template: Uint8Array,
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    exportOptions?: ExportFormat,
    compilationOptions?: CompilationMode,
    pages?: PageRange,
    limits?: ZipLimits,
  ): ExportOnceResult {
    const result = exportTemplateOnce(
      template,
      Object.fromEntries(jsonInputs ?? new Map<string, string>()),
      Template.convertBlobInputs(blobInputs ?? new Map<string, BlobInput>()),
      Template.mapCompilationMode(
        compilationOptions ?? CompilationMode.Production,
      ),
      JSON.stringify(exportOptions ?? Pdf),
      serializePageRange(pages),
      limits,
    );
    return { document: result.data, warnings: result.warnings ?? undefined };
  }

  /**
   * Compile and export a template in a single native call on a background
   * thread, without caching.
   *
   * Unlike {@link exportOnce}, this does not block the Node.js event loop.
   * @param template - the packed Oicana template file
   * @param jsonInputs - JSON inputs for the template (defaults to empty map)
   * @param blobInputs - Blob inputs for the template (defaults to empty map)
   * @param exportOptions - Export format specification (defaults to PDF)
   * @param compilationOptions - Compilation mode (defaults to Production)
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   * @param limits - limits for reading the template zip (defaults apply when omitted)
   */
  public static async exportOnceAsync(
    template: Uint8Array,
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    exportOptions?: ExportFormat,
    compilationOptions?: CompilationMode,
    pages?: PageRange,
    limits?: ZipLimits,
  ): Promise<ExportOnceResult> {
    const result = await exportTemplateOnceAsync(
      template,
      Object.fromEntries(jsonInputs ?? new Map<string, string>()),
      Template.convertBlobInputs(blobInputs ?? new Map<string, BlobInput>()),
      Template.mapCompilationMode(
        compilationOptions ?? CompilationMode.Production,
      ),
      JSON.stringify(exportOptions ?? Pdf),
      serializePageRange(pages),
      limits,
    );
    return { document: result.data, warnings: result.warnings ?? undefined };
  }

  /**
   * Compile the template and export it to a PDF file, without inputs, in
   * production mode.
   */
  public export(): Uint8Array;

  /**
   * Compile the template with the given inputs and export it to a PDF file in
   * production mode.
   * @param jsonInputs
   * @param blobInputs
   */
  public export(
    jsonInputs: Map<string, string>,
    blobInputs: Map<string, BlobInput>,
  ): Uint8Array;

  /**
   * Compile the template with the given inputs and export it in the given format.
   * @param jsonInputs
   * @param blobInputs
   * @param exportOptions
   */
  public export(
    jsonInputs: Map<string, string>,
    blobInputs: Map<string, BlobInput>,
    exportOptions: ExportFormat,
  ): Uint8Array;

  /**
   * Compile the template with the given inputs and export it in the given format.
   * @param jsonInputs
   * @param blobInputs
   * @param exportOptions
   * @param compilationOptions
   */
  public export(
    jsonInputs: Map<string, string>,
    blobInputs: Map<string, BlobInput>,
    exportOptions: ExportFormat,
    compilationOptions: CompilationMode,
  ): Uint8Array;

  /**
   * Compile the template with the given inputs and export a range of pages
   * @param jsonInputs
   * @param blobInputs
   * @param exportOptions
   * @param compilationOptions
   * @param pages
   */
  public export(
    jsonInputs: Map<string, string>,
    blobInputs: Map<string, BlobInput>,
    exportOptions: ExportFormat,
    compilationOptions: CompilationMode,
    pages: PageRange,
  ): Uint8Array;

  /**
   * Compile the template and export it in a single call, then free the document.
   *
   * To export the document
   * several times (multiple formats, page ranges, or individual pages) from a
   * single compilation, use {@link compile} and call `export` on the returned
   * {@link CompiledDocument}.
   * @param jsonInputs - JSON inputs for the template (defaults to empty map)
   * @param blobInputs - Blob inputs for the template (defaults to empty map)
   * @param exportOptions - Export format specification (defaults to PDF)
   * @param compilationOptions - Compilation mode (defaults to Production)
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   */
  public export(
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    exportOptions?: ExportFormat,
    compilationOptions?: CompilationMode,
    pages?: PageRange,
  ): Uint8Array {
    return this.exportWith(
      exportOptions ?? Pdf,
      jsonInputs,
      blobInputs,
      compilationOptions,
      pages,
    );
  }

  /**
   * Compile the template and export it in a single call on a background thread,
   * then free the document.
   *
   * Unlike {@link export}, this does not block the Node.js event loop while the
   * compilation and export run.
   * @param jsonInputs - JSON inputs for the template (defaults to empty map)
   * @param blobInputs - Blob inputs for the template (defaults to empty map)
   * @param exportOptions - Export format specification (defaults to PDF)
   * @param compilationOptions - Compilation mode (defaults to Production)
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   */
  public exportAsync(
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    exportOptions?: ExportFormat,
    compilationOptions?: CompilationMode,
    pages?: PageRange,
  ): Promise<Uint8Array> {
    return this.exportWithAsync(
      exportOptions ?? Pdf,
      jsonInputs,
      blobInputs,
      compilationOptions,
      pages,
    );
  }

  /**
   * Compile the template and export it to PDF in a single call, then free the
   * document.
   * Tagging will be automatically turned off when exporting a subset of pages.
   * @param jsonInputs - JSON inputs for the template (defaults to empty map)
   * @param blobInputs - Blob inputs for the template (defaults to empty map)
   * @param compilationOptions - Compilation mode (defaults to Production)
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   */
  public exportPdf(
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
    pages?: PageRange,
  ): Uint8Array {
    return this.exportWith(
      Pdf,
      jsonInputs,
      blobInputs,
      compilationOptions,
      pages,
    );
  }

  /**
   * Compile the template and export it to PDF in a single call on a background
   * thread, then free the document. The Node.js event loop stays free while the
   * work runs.
   * Tagging will be automatically turned off when exporting a subset of pages.
   * @param jsonInputs - JSON inputs for the template (defaults to empty map)
   * @param blobInputs - Blob inputs for the template (defaults to empty map)
   * @param compilationOptions - Compilation mode (defaults to Production)
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   */
  public exportPdfAsync(
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
    pages?: PageRange,
  ): Promise<Uint8Array> {
    return this.exportWithAsync(
      Pdf,
      jsonInputs,
      blobInputs,
      compilationOptions,
      pages,
    );
  }

  /**
   * Compile the template and export it to PNG in a single call, then free the
   * document.
   * Multiple pages are merged into a single, vertically stacked image.
   * @param jsonInputs - JSON inputs for the template (defaults to empty map)
   * @param blobInputs - Blob inputs for the template (defaults to empty map)
   * @param compilationOptions - Compilation mode (defaults to Production)
   * @param pixelsPerPt - resolution in pixels per point (defaults to 1.0)
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   */
  public exportPng(
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
    pixelsPerPt = 1.0,
    pages?: PageRange,
  ): Uint8Array {
    return this.exportWith(
      Png(pixelsPerPt),
      jsonInputs,
      blobInputs,
      compilationOptions,
      pages,
    );
  }

  /**
   * Compile the template and export it to PNG in a single call on a background
   * thread, then free the document. The Node.js event loop stays free while the
   * work runs.
   * Multiple pages are merged into a single, vertically stacked image.
   * @param jsonInputs - JSON inputs for the template (defaults to empty map)
   * @param blobInputs - Blob inputs for the template (defaults to empty map)
   * @param compilationOptions - Compilation mode (defaults to Production)
   * @param pixelsPerPt - resolution in pixels per point (defaults to 1.0)
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   */
  public exportPngAsync(
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
    pixelsPerPt = 1.0,
    pages?: PageRange,
  ): Promise<Uint8Array> {
    return this.exportWithAsync(
      Png(pixelsPerPt),
      jsonInputs,
      blobInputs,
      compilationOptions,
      pages,
    );
  }

  /**
   * Compile the template and export it to SVG in a single call, then free the
   * document.
   * @param jsonInputs - JSON inputs for the template (defaults to empty map)
   * @param blobInputs - Blob inputs for the template (defaults to empty map)
   * @param compilationOptions - Compilation mode (defaults to Production)
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   */
  public exportSvg(
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
    pages?: PageRange,
  ): Uint8Array {
    return this.exportWith(
      Svg,
      jsonInputs,
      blobInputs,
      compilationOptions,
      pages,
    );
  }

  /**
   * Compile the template and export it to SVG in a single call on a background
   * thread, then free the document. The Node.js event loop stays free while the
   * work runs.
   * @param jsonInputs - JSON inputs for the template (defaults to empty map)
   * @param blobInputs - Blob inputs for the template (defaults to empty map)
   * @param compilationOptions - Compilation mode (defaults to Production)
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   */
  public exportSvgAsync(
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
    pages?: PageRange,
  ): Promise<Uint8Array> {
    return this.exportWithAsync(
      Svg,
      jsonInputs,
      blobInputs,
      compilationOptions,
      pages,
    );
  }

  private exportWith(
    format: ExportFormat,
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
    pages?: PageRange,
  ): Uint8Array {
    const document = this.compileToDocumentId(
      jsonInputs,
      blobInputs,
      compilationOptions,
    );
    try {
      return exportDocument(
        document,
        JSON.stringify(format),
        serializePageRange(pages),
      );
    } finally {
      removeDocument(document);
    }
  }

  private async exportWithAsync(
    format: ExportFormat,
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
    pages?: PageRange,
  ): Promise<Uint8Array> {
    const document = await this.compileToDocumentIdAsync(
      jsonInputs,
      blobInputs,
      compilationOptions,
    );
    try {
      return await exportDocumentAsync(
        document,
        JSON.stringify(format),
        serializePageRange(pages),
      );
    } finally {
      removeDocument(document);
    }
  }

  /**
   * Compile the template and return a handle to the compiled document.
   *
   * The document is kept in memory so it can be exported one or more times
   * (whole document, a page range, or individual pages) without re-compiling.
   * Call `dispose()` on the returned document (or use `using`) to free it. For a
   * single one-shot export, prefer {@link export}.
   * @param jsonInputs - JSON inputs for the template (defaults to empty map)
   * @param blobInputs - Blob inputs for the template (defaults to empty map)
   * @param compilationOptions - Compilation mode (defaults to Production)
   */
  public compile(
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
  ): CompiledDocument {
    const documentId = this.compileToDocumentId(
      jsonInputs,
      blobInputs,
      compilationOptions,
    );
    return new CompiledDocument(documentId);
  }

  /**
   * Compile the template on a background thread and return a handle to the
   * compiled document.
   *
   * Unlike {@link compile}, this does not block the Node.js event loop while
   * the compilation runs. Call `dispose()` on the returned document (or use
   * `using`) to free it. For a single one-shot export, prefer
   * {@link exportAsync}.
   * @param jsonInputs - JSON inputs for the template (defaults to empty map)
   * @param blobInputs - Blob inputs for the template (defaults to empty map)
   * @param compilationOptions - Compilation mode (defaults to Production)
   */
  public async compileAsync(
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
  ): Promise<CompiledDocument> {
    const documentId = await this.compileToDocumentIdAsync(
      jsonInputs,
      blobInputs,
      compilationOptions,
    );
    return new CompiledDocument(documentId);
  }

  private compileToDocumentId(
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
  ): string {
    const documentId = compileTemplate(
      this.template,
      Object.fromEntries(jsonInputs ?? new Map<string, string>()),
      Template.convertBlobInputs(blobInputs ?? new Map<string, BlobInput>()),
      Template.mapCompilationMode(
        compilationOptions ?? CompilationMode.Production,
      ),
    );
    this.lastWarnings = getWarnings(documentId) ?? undefined;
    return documentId;
  }

  private async compileToDocumentIdAsync(
    jsonInputs?: Map<string, string>,
    blobInputs?: Map<string, BlobInput>,
    compilationOptions?: CompilationMode,
  ): Promise<string> {
    const documentId = await compileTemplateAsync(
      this.template,
      Object.fromEntries(jsonInputs ?? new Map<string, string>()),
      Template.convertBlobInputs(blobInputs ?? new Map<string, BlobInput>()),
      Template.mapCompilationMode(
        compilationOptions ?? CompilationMode.Production,
      ),
    );
    this.lastWarnings = getWarnings(documentId) ?? undefined;
    return documentId;
  }

  /**
   * Warnings produced by the most recent compilation (constructor warm-up, or a
   * `compile()` / `export()` call), or `undefined` if there were none.
   */
  public warnings(): string | undefined {
    return this.lastWarnings;
  }

  /**
   * The template's manifest.
   *
   * The Typst package section und its Oicana
   * configuration with the input definitions.
   */
  public manifest(): TemplateManifest {
    return JSON.parse(nativeManifest(this.template));
  }

  /**
   * The source text of a file inside the template.
   * @param path - file path within the template
   */
  public source(path: string): string {
    return getSource(this.template, path);
  }

  /**
   * The raw bytes of a file inside the template.
   * @param path - file path within the template
   */
  public file(path: string): Uint8Array {
    return getFile(this.template, path);
  }

  /**
   * Enable or disable JSON schema validation for this template.
   *
   * When enabled (the default), JSON inputs are validated against their schemas
   * before compilation.
   * @param validate - whether to validate inputs against their JSON schemas
   */
  public setValidateInputs(validate: boolean): void {
    setValidateInputs(this.template, validate);
  }

  /**
   * Release resources associated with this template.
   * After calling dispose(), this template instance should not be used.
   */
  public dispose(): void {
    removeWorld(this.template);
  }

  /**
   * Enables use with the `using` keyword for automatic resource cleanup.
   */
  [Symbol.dispose](): void {
    this.dispose();
  }

  private static convertBlobInputs(
    blobInputs: Map<string, BlobInput>,
  ): Record<string, BlobWithMetadataNative> {
    return Object.fromEntries(
      Array.from(blobInputs.entries(), ([key, value]) => {
        const nativeValue = {
          bytes: value.data,
          meta:
            value.metadata === undefined
              ? '{}'
              : JSON.stringify(value.metadata),
        };
        return [key, nativeValue];
      }),
    );
  }

  private static mapCompilationMode(
    mode: CompilationMode,
  ): NativeCompilationMode {
    switch (mode) {
      case CompilationMode.Development:
        return NativeCompilationMode.Development;
      case CompilationMode.Production:
        return NativeCompilationMode.Production;
    }
  }
}
