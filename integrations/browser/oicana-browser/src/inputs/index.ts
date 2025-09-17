import { BlobInputDefinition, BlobWithMetadata } from './BlobInput';
import { JsonInputDefinition } from './JsonInput';

export { JsonInputDefinition, BlobInputDefinition, BlobWithMetadata };

export interface Inputs {
  json: JsonInputDefinition[];
  blob: BlobInputDefinition[];
}
