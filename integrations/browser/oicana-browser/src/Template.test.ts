import { readFile, writeFile } from 'node:fs/promises';
import { describe, it } from 'vitest';
import { CompilationMode } from './CompilationMode';
import { Png } from './ExportFormat';
import type { BlobWithMetadata } from './inputs';
import { Template } from './Template';

const asset = (file: string) => {
  return readFile(`../../../assets/${file}`);
};

describe('e2e test template', () => {
  it('development', async () => {
    const templateFile = await readFile(
      '../../../e2e_test_template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template('test', templateFile);

    const image = template.compile(
      new Map(),
      new Map(),
      Png(1),
      CompilationMode.Development,
    );

    await writeFile('testOutput/development.png', image);
  });

  it('production', async () => {
    const templateFile = await readFile(
      '../../../e2e_test_template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template('test', templateFile);

    const blob = await asset('inputs/input.txt');
    const json = await asset('inputs/input.json');

    const blobInputs = new Map<string, BlobWithMetadata>();
    blobInputs.set('development-blob', {
      bytes: blob,
      meta: { image_format: 'jpeg', foo: 42, bar: ['input', 'two'] },
    });
    const jsonInputs = new Map<string, string>();
    jsonInputs.set('development-json', json.toString());

    const image = template.compile(jsonInputs, blobInputs, Png(1));

    await writeFile('testOutput/production.png', image);
  });

  it('all-inputs', async () => {
    const templateFile = await readFile(
      '../../../e2e_test_template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template('test', templateFile);

    const blob = await asset('inputs/input.txt');
    const json = await asset('inputs/input.json');

    const blobInputs = new Map<string, BlobWithMetadata>();
    blobInputs.set('default-blob', {
      bytes: blob,
      meta: { image_format: 'jpeg', foo: 42, bar: ['input', 'two'] },
    });
    blobInputs.set('development-blob', {
      bytes: blob,
      meta: { image_format: 'jpeg', foo: 43, bar: ['input', 'two'] },
    });
    blobInputs.set('both-blob', {
      bytes: blob,
      meta: { image_format: 'jpeg', foo: 44, bar: ['input', 'two'] },
    });
    const jsonInputs = new Map<string, string>();
    jsonInputs.set('default-json', json.toString());
    jsonInputs.set('development-json', json.toString());
    jsonInputs.set('both-json', json.toString());

    const image = template.compile(jsonInputs, blobInputs, Png(1));

    await writeFile('testOutput/all-inputs.png', image);
  });

  it('does not throw if inputs are objects, not Maps', async () => {
    const templateFile = await readFile(
      '../../../e2e_test_template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template('test', templateFile);

    const blob = await asset('inputs/input.txt');
    const json = await asset('inputs/input.json');

    const blobInputs = {
      'development-blob': {
        bytes: blob,
        meta: { image_format: 'jpeg', foo: 43, bar: ['input', 'two'] },
      },
    };
    const jsonInputs = {
      'development-json': json.toString(),
    };

    template.compile(
      jsonInputs as unknown as Map<string, string>,
      blobInputs as unknown as Map<string, BlobWithMetadata>,
      Png(1),
    );
  });
});
