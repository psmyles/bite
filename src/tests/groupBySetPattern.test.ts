import { describe, it, expect } from 'vitest';
import { groupBySetPattern } from '../main/pipeline/graph-utils.js';

const paths = (names: string[]) => names.map((n) => `/images/${n}`);

describe('groupBySetPattern', () => {
  it('returns empty map for empty input', () => {
    expect(groupBySetPattern([], '', ['_AO'])).toEqual(new Map());
  });

  it('returns empty map when suffixes list is empty', () => {
    const result = groupBySetPattern(paths(['T_Jeep_body_AO.png']), 'T_', []);
    expect(result.size).toBe(0);
  });

  it('returns empty map when no file matches prefix', () => {
    const result = groupBySetPattern(paths(['Other_body_AO.png']), 'T_', ['_AO']);
    expect(result.size).toBe(0);
  });

  it('returns empty map when no file matches any suffix', () => {
    const result = groupBySetPattern(paths(['T_body_normal.png']), 'T_', ['_AO', '_roughness']);
    expect(result.size).toBe(0);
  });

  it('groups one complete set', () => {
    const files = paths(['T_body_AO.png', 'T_body_roughness.png', 'T_body_metallic.png']);
    const result = groupBySetPattern(files, 'T_', ['_AO', '_roughness', '_metallic']);
    expect(result.size).toBe(1);
    const group = result.get('body')!;
    expect(group['_AO']).toContain('T_body_AO.png');
    expect(group['_roughness']).toContain('T_body_roughness.png');
    expect(group['_metallic']).toContain('T_body_metallic.png');
  });

  it('groups multiple sets correctly', () => {
    const files = paths(['T_body_AO.png', 'T_body_roughness.png', 'T_glass_AO.png', 'T_glass_roughness.png']);
    const result = groupBySetPattern(files, 'T_', ['_AO', '_roughness']);
    expect(result.size).toBe(2);
    expect(result.has('body')).toBe(true);
    expect(result.has('glass')).toBe(true);
  });

  it('handles incomplete sets (missing a suffix)', () => {
    const files = paths(['T_body_AO.png']); // no roughness
    const result = groupBySetPattern(files, 'T_', ['_AO', '_roughness']);
    expect(result.size).toBe(1);
    const group = result.get('body')!;
    expect(group['_AO']).toContain('T_body_AO.png');
    expect(group['_roughness']).toBeUndefined();
  });

  it('empty prefix matches all files', () => {
    const files = paths(['body_AO.png', 'glass_AO.png']);
    const result = groupBySetPattern(files, '', ['_AO']);
    expect(result.size).toBe(2);
    expect(result.has('body')).toBe(true);
    expect(result.has('glass')).toBe(true);
  });

  it('skips empty suffix strings', () => {
    const files = paths(['T_body_AO.png']);
    const result = groupBySetPattern(files, 'T_', ['', '_AO']);
    expect(result.size).toBe(1);
    expect(result.get('body')!['_AO']).toContain('T_body_AO.png');
  });

  it('each file only matches the first matching suffix', () => {
    // '_AO' is a suffix of '_extra_AO' — file should match '_AO', not be double-counted
    const files = paths(['T_body_AO.png', 'T_body_extra_AO.png']);
    const result = groupBySetPattern(files, 'T_', ['_AO']);
    // 'body' matches _AO, 'body_extra' also matches _AO — two separate middles
    expect(result.size).toBe(2);
    expect(result.has('body')).toBe(true);
    expect(result.has('body_extra')).toBe(true);
  });

  it('extension is stripped before matching', () => {
    const files = paths(['T_body_AO.jpg']);
    const result = groupBySetPattern(files, 'T_', ['_AO']);
    expect(result.size).toBe(1);
    expect(result.get('body')!['_AO']).toContain('T_body_AO.jpg');
  });

  it('middle can be empty string (file is exactly prefix+suffix)', () => {
    const files = paths(['T__AO.png']);
    const result = groupBySetPattern(files, 'T_', ['_AO']);
    expect(result.size).toBe(1);
    expect(result.has('')).toBe(true);
  });
});
