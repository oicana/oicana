import { readFile } from 'node:fs/promises';
import * as zlib from 'node:zlib';
import { describe, expect, it } from 'vitest';
import { CompilationMode } from './CompilationMode';
import { configureDiagnosticColor, DiagnosticColor } from './DiagnosticColor';
import { Svg } from './ExportFormat';
import { Template } from './Template';

const templateFile = () => {
  return readFile('../../../e2e-tests/template/oicana-e2e-test-x.y.z.zip');
};

const minimalManifest = [
  '[package]',
  'name = "export-once-test"',
  'version = "0.1.0"',
  'entrypoint = "main.typ"',
  '',
  '[tool.oicana]',
  'manifest_version = 1',
  '',
].join('\n');

/** Build a stored (uncompressed) zip with the given entries. */
function packTemplate(entries: Record<string, string>): Uint8Array {
  const localParts: Buffer[] = [];
  const centralParts: Buffer[] = [];
  let offset = 0;

  for (const [name, content] of Object.entries(entries)) {
    const nameBuffer = Buffer.from(name, 'utf-8');
    const data = Buffer.from(content, 'utf-8');
    const crc = zlib.crc32(data);

    const localHeader = Buffer.alloc(30);
    localHeader.writeUInt32LE(0x04034b50, 0);
    localHeader.writeUInt16LE(20, 4);
    localHeader.writeUInt32LE(crc, 14);
    localHeader.writeUInt32LE(data.length, 18);
    localHeader.writeUInt32LE(data.length, 22);
    localHeader.writeUInt16LE(nameBuffer.length, 26);
    localParts.push(localHeader, nameBuffer, data);

    const centralHeader = Buffer.alloc(46);
    centralHeader.writeUInt32LE(0x02014b50, 0);
    centralHeader.writeUInt16LE(20, 4);
    centralHeader.writeUInt16LE(20, 6);
    centralHeader.writeUInt32LE(crc, 16);
    centralHeader.writeUInt32LE(data.length, 20);
    centralHeader.writeUInt32LE(data.length, 24);
    centralHeader.writeUInt16LE(nameBuffer.length, 28);
    centralHeader.writeUInt32LE(offset, 42);
    centralParts.push(centralHeader, nameBuffer);

    offset += 30 + nameBuffer.length + data.length;
  }

  const centralSize = centralParts.reduce((sum, part) => sum + part.length, 0);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(Object.keys(entries).length, 8);
  end.writeUInt16LE(Object.keys(entries).length, 10);
  end.writeUInt32LE(centralSize, 12);
  end.writeUInt32LE(offset, 16);

  return Buffer.concat([...localParts, ...centralParts, end]);
}

describe('exportOnce', () => {
  it('exports without warnings', async () => {
    const result = Template.exportOnce(
      await templateFile(),
      new Map(),
      new Map(),
      undefined,
      CompilationMode.Development,
    );

    expect(Buffer.from(result.document.slice(0, 4)).toString('ascii')).toBe(
      '%PDF',
    );
    expect(result.warnings).toBeUndefined();
  });

  it('exports asynchronously', async () => {
    const result = await Template.exportOnceAsync(
      await templateFile(),
      new Map(),
      new Map(),
      undefined,
      CompilationMode.Development,
    );

    expect(Buffer.from(result.document.slice(0, 4)).toString('ascii')).toBe(
      '%PDF',
    );
    expect(result.warnings).toBeUndefined();
  });

  it('surfaces warnings', () => {
    const template = packTemplate({
      'typst.toml': minimalManifest,
      'main.typ': '#set text(font: "NonexistentFontExportOnce")\nContent',
    });

    const result = Template.exportOnce(
      template,
      new Map(),
      new Map(),
      Svg,
      CompilationMode.Development,
    );

    expect(Buffer.from(result.document).toString('utf-8')).toContain('<svg');
    expect(result.warnings).toContain('NonexistentFontExportOnce');
  });

  it('colors diagnostics only when configured', () => {
    const template = packTemplate({
      'typst.toml': minimalManifest,
      'main.typ': '#set text(font: "NonexistentFontColor")\nContent',
    });
    const warnings = () =>
      Template.exportOnce(template, new Map(), new Map(), Svg).warnings;

    try {
      configureDiagnosticColor(DiagnosticColor.Ansi);
      expect(warnings()).toContain('\u001b[');
    } finally {
      configureDiagnosticColor(DiagnosticColor.None);
    }
    expect(warnings()).not.toContain('\u001b[');
  });

  it('enforces zip limits', async () => {
    const file = await templateFile();

    expect(() =>
      Template.exportOnce(
        file,
        new Map(),
        new Map(),
        undefined,
        CompilationMode.Development,
        undefined,
        { maxEntries: 1 },
      ),
    ).toThrowError(/entries/);
  });

  it('registration enforces zip limits', async () => {
    const file = await templateFile();

    expect(
      () =>
        new Template(file, new Map(), new Map(), CompilationMode.Development, {
          maxEntries: 1,
        }),
    ).toThrowError(/entries/);
  });
});
