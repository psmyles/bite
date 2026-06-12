import { describe, it, expect } from 'vitest';
import { compareSemver, extractImgplexPath } from '../../electron/update-utils.js';

describe('compareSemver', () => {
  it('orders by major, then minor, then patch', () => {
    expect(compareSemver('2.0.0', '1.9.9')).toBeGreaterThan(0);
    expect(compareSemver('1.2.0', '1.1.9')).toBeGreaterThan(0);
    expect(compareSemver('1.1.2', '1.1.1')).toBeGreaterThan(0);
  });

  it('treats equal versions as 0', () => {
    expect(compareSemver('1.2.3', '1.2.3')).toBe(0);
  });

  it('ignores a leading v', () => {
    expect(compareSemver('v1.2.3', '1.2.3')).toBe(0);
    expect(compareSemver('v2.0.0', 'v1.0.0')).toBeGreaterThan(0);
  });

  it('handles pre-release suffixes without producing NaN', () => {
    // '1.2.3-beta' must not make the patch NaN (which would wrongly read as "no update").
    expect(compareSemver('1.2.4', '1.2.3-beta')).toBeGreaterThan(0);
    expect(compareSemver('1.2.3-beta', '1.2.3')).toBe(0);
  });
});

describe('extractImgplexPath', () => {
  it('finds a .imgplex arg in dev mode (start index 2)', () => {
    const argv = ['electron', 'main.js', 'C:\\work\\flow.imgplex'];
    expect(extractImgplexPath(argv, false)).toBe('C:\\work\\flow.imgplex');
  });

  it('finds a .imgplex arg in packaged mode (start index 1)', () => {
    const argv = ['imgplex.exe', 'C:\\work\\flow.imgplex'];
    expect(extractImgplexPath(argv, true)).toBe('C:\\work\\flow.imgplex');
  });

  it('skips flags', () => {
    const argv = ['imgplex.exe', '--some-flag', 'flow.imgplex'];
    expect(extractImgplexPath(argv, true)).toBe('flow.imgplex');
  });

  it('returns null when no .imgplex arg is present', () => {
    expect(extractImgplexPath(['imgplex.exe', 'image.png'], true)).toBeNull();
  });

  it('does not treat the exe name as a path (packaged starts at 1)', () => {
    expect(extractImgplexPath(['weird.imgplex'], true)).toBeNull();
  });
});
