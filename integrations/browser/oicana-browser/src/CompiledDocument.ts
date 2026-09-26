import {
  document_pages,
  export_document,
  get_warnings,
  remove_document,
} from '@oicana/browser-wasm';
import { type ExportFormat, Pdf, Png, Svg } from './ExportFormat.js';
import type { PageRange } from './PageRange.js';

/**
 * Size of a single document page, in typographic points (pt).
 */
export interface PageSize {
  /** Page width in points. */
  width: number;
  /** Page height in points. */
  height: number;
}

/**
 * A compiled document. Its pages can be exported on
 * demand without re-compiling.
 *
 * Obtain one via {@link Template.compile}. Call {@link dispose} (or
 * use the `using` keyword) to release the underlying document.
 */
export class CompiledDocument implements Disposable {
  private documentId: string | undefined;

  /** Sizes (in points) of every page, in document order. */
  public readonly pages: ReadonlyArray<PageSize>;

  /**
   * Warnings produced by the compilation of this document, or `undefined` if
   * there were none.
   */
  public readonly warnings: string | undefined;

  /**
   * @internal Construct via {@link Template.compile}.
   */
  public constructor(documentId: string) {
    this.documentId = documentId;
    this.pages = JSON.parse(document_pages(documentId)) as PageSize[];
    this.warnings = get_warnings(documentId);
  }

  /** Number of pages in the document. */
  public get pageCount(): number {
    return this.pages.length;
  }

  /**
   * Export the document in the given format (defaults to PDF), optionally
   * restricted to a range of pages.
   * @param format - export format specification
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   */
  public export(
    format: ExportFormat = Pdf,
    pages?: PageRange,
  ): Uint8Array<ArrayBuffer> {
    if (this.documentId === undefined) {
      throw new Error('CompiledDocument has already been disposed');
    }
    return export_document(
      this.documentId,
      format,
      pages,
    ) as Uint8Array<ArrayBuffer>;
  }

  /**
   * Export the document to PDF, optionally restricted to a range of pages.
   * Tagging will be automatically turned off when exporting a subset of pages.
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   */
  public exportPdf(pages?: PageRange): Uint8Array<ArrayBuffer> {
    return this.export(Pdf, pages);
  }

  /**
   * Export the document to PNG, optionally restricted to a range of pages.
   * Multiple pages are merged into a single, vertically stacked image.
   * @param pixelsPerPt - resolution in pixels per point (defaults to 1.0)
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   */
  public exportPng(
    pixelsPerPt = 1.0,
    pages?: PageRange,
  ): Uint8Array<ArrayBuffer> {
    return this.export(Png(pixelsPerPt), pages);
  }

  /**
   * Export the document to SVG, optionally restricted to a range of pages.
   * @param pages - 0-based, inclusive page range (defaults to the whole document)
   */
  public exportSvg(pages?: PageRange): Uint8Array<ArrayBuffer> {
    return this.export(Svg, pages);
  }

  /**
   * Release the cached document. After calling dispose() this instance must not
   * be used anymore.
   */
  public dispose(): void {
    if (this.documentId !== undefined) {
      remove_document(this.documentId);
      this.documentId = undefined;
    }
  }

  /**
   * Enables use with the `using` keyword for automatic resource cleanup.
   */
  [Symbol.dispose](): void {
    this.dispose();
  }
}
