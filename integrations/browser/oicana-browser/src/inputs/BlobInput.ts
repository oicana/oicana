export interface BlobWithMetadata {
  bytes: Uint8Array;
  meta?: {
    image_format?: string;
    // biome-ignore lint/suspicious/noExplicitAny: Really anything that can pe serilized and deserialized into a Typst Dict is OK
    [key: string]: any;
  };
}

export interface BlobInputDefinition {
  key: string;
  default: {
    file: string;
    meta?: {
      image_format?: string;
      // biome-ignore lint/suspicious/noExplicitAny: Really anything that can pe serilized and deserialized into a Typst Dict is OK
      [key: string]: any;
    };
  };
}
