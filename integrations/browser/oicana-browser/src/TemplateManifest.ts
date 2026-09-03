import type { BlobMetadata } from './BlobInput.js';

/** A template's manifest. */
export interface TemplateManifest {
  /** The Typst package section of the manifest. */
  package: PackageInfo;
  /** The Oicana section of the manifest. */
  oicana: OicanaConfig;
}

/** The Typst package a template is. */
export interface PackageInfo {
  name: string;
  version: string;
  /** File the compilation starts at. */
  entrypoint: string;
  authors: string[];
  license: string | null;
  description: string | null;
  homepage: string | null;
  repository: string | null;
}

/** The Oicana configuration of a template. */
export interface OicanaConfig {
  /** Version of the manifest format. */
  manifestVersion: number;
  /** The inputs the template declares, in manifest order. */
  inputs: InputDefinition[];
  /** Whether JSON inputs are validated against their schemas by default. */
  validateJsonInputsByDefault: boolean;
  export: ExportConfig;
  /** Fonts the template expects from its host. */
  fonts: FontConfig;
}

/** An input a template declares, discriminated by `type`. */
export type InputDefinition = JsonInputDefinition | BlobInputDefinition;

/** An input taking a JSON value. */
export interface JsonInputDefinition {
  type: 'json';
  /** Key the input is supplied and used under. */
  key: string;
  /** Whether a value of this input is required for compilation. */
  required: boolean;
  /**
   * File in the template holding the value used when none is supplied.
   * In development mode, `development` takes precedence.
   */
  default: string | null;
  /** File in the template holding the value used in development mode when none is supplied. */
  development: string | null;
  /** File in the template holding the JSON schema of this input. */
  schema: string | null;
  /** Whether values are validated against the schema. */
  validate: boolean;
}

/** An input taking arbitrary bytes. */
export interface BlobInputDefinition {
  type: 'blob';
  /** Key the input is supplied and used under. */
  key: string;
  /** Whether a value of this input is required for compilation. */
  required: boolean;
  /**
   * Blob used when no value is supplied.
   * In development mode, `development` takes precedence.
   */
  default: BlobFallback | null;
  /** Blob used in development mode when no value is supplied. */
  development: BlobFallback | null;
}

/** A blob from the template, used when no value is supplied. */
export interface BlobFallback {
  /** File in the template holding the blob. */
  file: string;
  /** Metadata passed to the template along with the blob. */
  meta: BlobMetadata | null;
}

/** How compiled documents are exported. */
export interface ExportConfig {
  pdf: PdfExportConfig;
}

/** How documents are exported to PDF. */
export interface PdfExportConfig {
  /** PDF standards the export conforms to, for example `a-3b`. */
  standards: string[];
  /** Whether the PDF is tagged for accessibility. */
  tagged: boolean;
}

/** Fonts a template expects from its host. */
export interface FontConfig {
  /** Font families the host has to register. */
  require: string[];
}
