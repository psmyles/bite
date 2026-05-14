import { describe, it, expect } from 'vitest';
import { buildCommandArgs, buildCommandArgsFromJs } from '../main/pipeline/command-builder.js';
import type { NodeDefinition } from '../shared/types.js';

function def(overrides: Partial<NodeDefinition> & { id: string }): NodeDefinition {
  return {
    label: overrides.id,
    category: 'test',
    inputs: [],
    outputs: [],
    params: [],
    ...overrides,
  };
}

// ── buildCommandArgs ──────────────────────────────────────────────────────────

describe('buildCommandArgs', () => {
  it('returns [] when no command_template', () => {
    expect(buildCommandArgs(def({ id: 'x' }), {})).toEqual([]);
  });

  it('returns [] for empty template', () => {
    expect(buildCommandArgs(def({ id: 'x', command_template: '' }), {})).toEqual([]);
  });

  it('splits plain template on whitespace', () => {
    const d = def({ id: 'x', command_template: '-resize 50%' });
    expect(buildCommandArgs(d, {})).toEqual(['-resize', '50%']);
  });

  it('interpolates a param value', () => {
    const d = def({
      id: 'x',
      command_template: '-quality {{quality}}',
      params: [{ name: 'quality', label: 'Quality', type: 'int', widget: 'slider', default: 90 }],
    });
    expect(buildCommandArgs(d, { quality: 75 })).toEqual(['-quality', '75']);
  });

  it('falls back to param default when key missing from params record', () => {
    const d = def({
      id: 'x',
      command_template: '-quality {{quality}}',
      params: [{ name: 'quality', label: 'Quality', type: 'int', widget: 'slider', default: 90 }],
    });
    expect(buildCommandArgs(d, {})).toEqual(['-quality', '90']);
  });

  it('replaces missing key with empty string when no default', () => {
    const d = def({ id: 'x', command_template: '-format {{fmt}}', params: [] });
    // No param named 'fmt', no default — becomes empty string, then filtered out
    expect(buildCommandArgs(d, {})).toEqual(['-format']);
  });

  it('interpolates multiple params in one template', () => {
    const d = def({
      id: 'x',
      command_template: '-resize {{w}}x{{h}}',
      params: [
        { name: 'w', label: 'W', type: 'int', widget: 'number', default: 512 },
        { name: 'h', label: 'H', type: 'int', widget: 'number', default: 512 },
      ],
    });
    expect(buildCommandArgs(d, { w: 1024, h: 768 })).toEqual(['-resize', '1024x768']);
  });

  it('collapses extra whitespace between tokens', () => {
    const d = def({ id: 'x', command_template: '-a   -b   -c' });
    expect(buildCommandArgs(d, {})).toEqual(['-a', '-b', '-c']);
  });

  it('handles params record value of 0 (falsy) correctly', () => {
    const d = def({
      id: 'x',
      command_template: '-threshold {{val}}',
      params: [{ name: 'val', label: 'Val', type: 'float', widget: 'slider', default: 50 }],
    });
    expect(buildCommandArgs(d, { val: 0 })).toEqual(['-threshold', '0']);
  });
});

// ── buildCommandArgsFromJs ────────────────────────────────────────────────────

describe('buildCommandArgsFromJs', () => {
  it('executes command_js and returns result', () => {
    const d = def({ id: 'x', command_js: 'return ["-quality", String(params.q)]' });
    expect(buildCommandArgsFromJs(d, { q: 80 })).toEqual(['-quality', '80']);
  });

  it('returns [] from command_js returning empty array', () => {
    const d = def({ id: 'x', command_js: 'return []' });
    expect(buildCommandArgsFromJs(d, {})).toEqual([]);
  });

  it('throws when command_js returns non-array', () => {
    const d = def({ id: 'x', command_js: 'return "bad"' });
    expect(() => buildCommandArgsFromJs(d, {})).toThrow();
  });

  it('throws when command_js returns array with non-strings', () => {
    const d = def({ id: 'x', command_js: 'return [1, 2]' });
    expect(() => buildCommandArgsFromJs(d, {})).toThrow();
  });

  it('command_js can access params', () => {
    const d = def({ id: 'x', command_js: 'return params.enabled ? ["-sharpen", "0x1"] : []' });
    expect(buildCommandArgsFromJs(d, { enabled: true })).toEqual(['-sharpen', '0x1']);
    expect(buildCommandArgsFromJs(d, { enabled: false })).toEqual([]);
  });

  it('command_js throwing propagates the error', () => {
    const d = def({ id: 'x', command_js: 'throw new Error("oops")' });
    expect(() => buildCommandArgsFromJs(d, {})).toThrow('oops');
  });
});
