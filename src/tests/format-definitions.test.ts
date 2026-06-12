import { describe, it, expect } from 'vitest';
import type { FormatDefinition } from '../shared/types.js';
import { buildFormatConvertArgs, getFormatExtension } from '../main/pipeline/command-builder.js';

// Load every format definition the same way command-builder does, so this table
// test protects every current and future format file (added 594f7d8 migration).
const rawDefs = import.meta.glob('../../format-definitions/*.json', { eager: true });
const defs = Object.values(rawDefs).map((m) => m as FormatDefinition);

function defaultParams(def: FormatDefinition): Record<string, unknown> {
  const params: Record<string, unknown> = {};
  for (const p of def.params ?? []) {
    params[(p as { name: string }).name] = (p as { default?: unknown }).default;
  }
  return params;
}

describe('format definitions', () => {
  it('discovers at least one format file', () => {
    expect(defs.length).toBeGreaterThan(0);
  });

  for (const def of defs) {
    describe(def.id, () => {
      it('args_js returns string[] for default params', () => {
        const args = buildFormatConvertArgs(def.id, defaultParams(def));
        expect(Array.isArray(args)).toBe(true);
        expect(args.every((a) => typeof a === 'string')).toBe(true);
      });

      it('getFormatExtension matches the definition and starts with a dot', () => {
        const ext = getFormatExtension(def.id);
        expect(ext).toBe(def.extension);
        expect(ext.startsWith('.')).toBe(true);
      });

      it('id lookup is case-insensitive', () => {
        expect(getFormatExtension(def.id.toLowerCase())).toBe(def.extension);
      });
    });
  }

  it('backward-compat: legacy shared `quality` maps to the per-format key', () => {
    // JPEG existed in the old quality system; passing the old `quality` should still
    // influence -quality even though the new key is jpeg_quality.
    const args = buildFormatConvertArgs('JPEG', { quality: 42 });
    const qi = args.indexOf('-quality');
    expect(qi).toBeGreaterThanOrEqual(0);
    expect(args[qi + 1]).toBe('42');
  });

  it('unknown format yields no args and a png fallback extension', () => {
    expect(buildFormatConvertArgs('NOPE', {})).toEqual([]);
    expect(getFormatExtension('NOPE')).toBe('.png');
  });
});
