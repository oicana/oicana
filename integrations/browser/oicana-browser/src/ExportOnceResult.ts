/**
 * Result of a one-shot template export.
 */
export interface ExportOnceResult {
  /** The exported document. */
  document: Uint8Array<ArrayBuffer>;
  /** Compilation warnings, or `undefined` if there were none. */
  warnings?: string;
}
