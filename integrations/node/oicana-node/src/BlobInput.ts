export interface BlobMetadata {
  image_format?: string;
  // biome-ignore lint/suspicious/noExplicitAny: Really anything that can pe serilized and deserialized into a Typst Dict is OK
  [key: string]: any;
}

export interface BlobInput {
  data: Uint8Array;
  metadata?: BlobMetadata;
}
