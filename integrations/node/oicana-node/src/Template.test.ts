import { readFile, writeFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import type { BlobInput } from './BlobInput';
import { CompilationMode } from './CompilationMode';
import { Png } from './ExportFormat';
import { PageRange } from './PageRange';
import { Template } from './Template';

const asset = (file: string) => {
  return readFile(`../../../assets/${file}`);
};

describe('e2e test template', () => {
  it('development', async () => {
    const templateFile = await readFile(
      '../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template(templateFile);

    const image = template.export(
      new Map(),
      new Map(),
      Png(1),
      CompilationMode.Development,
    );

    await writeFile('testOutput/development.png', image);
  });

  it('production', async () => {
    const templateFile = await readFile(
      '../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template(templateFile);

    const blob = await asset('inputs/input.txt');
    const json = await asset('inputs/input.json');

    const blobInputs = new Map<string, BlobInput>();
    blobInputs.set('development-blob', {
      data: blob,
      metadata: { image_format: 'jpeg', foo: 43, bar: ['input', 'two'] },
    });
    const jsonInputs = new Map<string, string>();
    jsonInputs.set('development-json', json.toString());

    const image = template.export(jsonInputs, blobInputs, Png(1));

    await writeFile('testOutput/production.png', image);
  });

  it('all-inputs', async () => {
    const templateFile = await readFile(
      '../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template(templateFile);

    const blob = await asset('inputs/input.txt');
    const json = await asset('inputs/input.json');

    const blobInputs = new Map<string, BlobInput>();
    blobInputs.set('default-blob', {
      data: blob,
      metadata: { image_format: 'jpeg', foo: 42, bar: ['input', 'two'] },
    });
    blobInputs.set('development-blob', {
      data: blob,
      metadata: { image_format: 'jpeg', foo: 43, bar: ['input', 'two'] },
    });
    blobInputs.set('both-blob', {
      data: blob,
      metadata: { image_format: 'jpeg', foo: 44, bar: ['input', 'two'] },
    });
    const jsonInputs = new Map<string, string>();
    jsonInputs.set('default-json', json.toString());
    jsonInputs.set('development-json', json.toString());
    jsonInputs.set('both-json', json.toString());

    const image = template.export(jsonInputs, blobInputs, Png(1));

    await writeFile('testOutput/all-inputs.png', image);
  });

  it('exposes the typed manifest', async () => {
    const templateFile = await readFile(
      '../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template(templateFile);

    const manifest = template.manifest();

    expect(manifest.package.name).toBe('oicana-e2e-test');
    expect(manifest.package.version).toBe('0.1.0');
    expect(manifest.oicana.manifestVersion).toBe(1);
    expect(manifest.oicana.validateJsonInputsByDefault).toBe(true);
    expect(manifest.oicana.export.pdf.standards).toEqual(['a-3b']);
    expect(manifest.oicana.fonts.require).toEqual([]);

    const keys = manifest.oicana.inputs.map((input) => input.key);
    expect(keys).toContain('development-json');

    const json = manifest.oicana.inputs.find(
      (input) => input.key === 'development-json',
    );
    expect(json?.type).toBe('json');
    if (json?.type !== 'json') throw new Error('expected a JSON input');
    expect(json.schema).toBe('input.schema.json');
    expect(json.development).toBe('development.json');
    expect(json.default).toBeNull();
    expect(json.validate).toBe(true);

    const blob = manifest.oicana.inputs.find(
      (input) => input.key === 'default-blob',
    );
    if (blob?.type !== 'blob') throw new Error('expected a blob input');
    expect(blob.default?.file).toBe('default.txt');
    expect(blob.default?.meta?.image_format).toBe('png');
    expect(blob.development).toBeNull();
  });

  it('explicit development mode allows compile with empty inputs', async () => {
    const templateFile = await readFile(
      '../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template(templateFile);

    template.export(new Map(), new Map(), Png(1), CompilationMode.Development);
  });

  it('export defaults to production mode', async () => {
    const templateFile = await readFile(
      '../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template(templateFile);

    expect(() => {
      template.export(new Map(), new Map(), Png(1));
    }).toThrow(/No value for the required input/);
  });

  it('can control compilation mode when registering', async () => {
    const templateFile = await readFile(
      '../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip',
    );
    expect(() => {
      new Template(
        templateFile,
        new Map(),
        new Map(),
        CompilationMode.Production,
      );
    }).toThrow(/No value for the required input/);
  });

  it('exports a compiled document handle in every format after disposing the template', async () => {
    const templateFile = await readFile(
      '../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template(templateFile);

    const document = template.compile(
      new Map(),
      new Map(),
      CompilationMode.Development,
    );

    template.dispose();

    expect(document.pageCount).toBeGreaterThan(0);
    const firstPage = PageRange.single(0);

    const pdf = document.exportPdf(firstPage);
    expect(new TextDecoder().decode(pdf.slice(0, 4))).toBe('%PDF');

    const png = document.export(Png(1), firstPage);
    expect(Array.from(png.slice(0, 4))).toEqual([0x89, 0x50, 0x4e, 0x47]);

    const svg = document.exportSvg(firstPage);
    expect(new TextDecoder().decode(svg)).toContain('<svg');

    const firstPagePng = document.exportPng(1, PageRange.single(0));
    expect(Array.from(firstPagePng.slice(0, 4))).toEqual([
      0x89, 0x50, 0x4e, 0x47,
    ]);

    document.dispose();
  });

  it('compiles and exports asynchronously', async () => {
    const templateFile = await readFile(
      '../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template(templateFile);

    const document = await template.compileAsync(
      new Map(),
      new Map(),
      CompilationMode.Development,
    );
    expect(document.pageCount).toBeGreaterThan(0);

    const pdf = await document.exportPdfAsync();
    expect(new TextDecoder().decode(pdf.slice(0, 4))).toBe('%PDF');

    const png = await document.exportPngAsync(1);
    expect(Array.from(png.slice(0, 4))).toEqual([0x89, 0x50, 0x4e, 0x47]);

    const svg = await document.exportSvgAsync();
    expect(new TextDecoder().decode(svg)).toContain('<svg');

    document.dispose();
    await expect(document.exportAsync()).rejects.toThrow(
      /already been disposed/,
    );

    const image = await template.exportPngAsync(
      new Map(),
      new Map(),
      CompilationMode.Development,
      1,
    );
    expect(Array.from(image.slice(0, 4))).toEqual([0x89, 0x50, 0x4e, 0x47]);

    template.dispose();
  });

  it('async export rejects with compilation errors', async () => {
    const templateFile = await readFile(
      '../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template(templateFile);

    await expect(
      template.exportAsync(new Map(), new Map(), Png(1)),
    ).rejects.toThrow(/No value for the required input/);

    template.dispose();
  });
});

describe('async registration', () => {
  it('creates a template on a background thread', async () => {
    const templateFile = await readFile(
      '../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip',
    );
    const template = await Template.create(templateFile);

    const image = await template.exportPngAsync(
      new Map(),
      new Map(),
      CompilationMode.Development,
      1,
    );
    expect(Array.from(image.slice(0, 4))).toEqual([0x89, 0x50, 0x4e, 0x47]);

    template.dispose();
  });

  it('rejects with registration errors', async () => {
    const notAZip = new Uint8Array([1, 2, 3, 4]);
    await expect(Template.create(notAZip)).rejects.toThrow(
      /Failed to read template/,
    );
  });
});

describe('template introspection', () => {
  it('exposes the manifest, sources and files', async () => {
    const templateFile = await readFile(
      '../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip',
    );
    const template = new Template(templateFile);

    expect(
      template.manifest().oicana.inputs.map((input) => input.key),
    ).toContain('development-blob');

    expect(template.source('/main.typ')).toContain('development-blob');

    const file = template.file('/default.txt');
    expect(file.length).toBeGreaterThan(0);

    expect(() => template.source('/nonexistent.typ')).toThrow(
      /Failed to load source/,
    );

    template.dispose();
  });
});

describe('constructor input validation', () => {
  it('rejects values that are not a Uint8Array', () => {
    expect(() => new Template({} as never)).toThrow(/must be a Uint8Array/);
    expect(() => new Template([1, 2, 3] as never)).toThrow(
      /must be a Uint8Array/,
    );
  });
});

describe('manifest compatibility', () => {
  it('refuses a template packed by a newer Oicana', async () => {
    const templateFile = await readFile(
      '../../../assets/templates/future-manifest-0.1.0.zip',
    );

    expect(() => new Template(templateFile)).toThrow(/manifest_version 99/);
  });
});
