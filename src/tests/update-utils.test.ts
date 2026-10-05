import { describe, it, expect } from 'vitest';
import { compareSemver, extractWorkflowPath } from '../../electron/update-utils.js';

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

describe('extractWorkflowPath', () => {
  it('finds a .bite arg in dev mode (start index 2)', () => {
    const argv = ['electron', 'main.js', 'C:\\work\\flow.bite'];
    expect(extractWorkflowPath(argv, false)).toBe('C:\\work\\flow.bite');
  });

  it('finds a .bite arg in packaged mode (start index 1)', () => {
    const argv = ['bite-gui.exe', 'C:\\work\\flow.bite'];
    expect(extractWorkflowPath(argv, true)).toBe('C:\\work\\flow.bite');
  });

  it('skips flags', () => {
    const argv = ['bite-gui.exe', '--some-flag', 'flow.bite'];
    expect(extractWorkflowPath(argv, true)).toBe('flow.bite');
  });

  it('returns null when no .bite arg is present', () => {
    expect(extractWorkflowPath(['bite-gui.exe', 'image.png'], true)).toBeNull();
  });

  it('does not treat the exe name as a path (packaged starts at 1)', () => {
    expect(extractWorkflowPath(['weird.bite'], true)).toBeNull();
  });
});
