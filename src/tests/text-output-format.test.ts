import { describe, it, expect } from 'vitest';
import { valueToString } from '../main/pipeline/text-output.js';

describe('valueToString', () => {
  it('renders null/undefined as empty string', () => {
    expect(valueToString(null)).toBe('');
    expect(valueToString(undefined)).toBe('');
  });

  it('renders booleans as true/false', () => {
    expect(valueToString(true)).toBe('true');
    expect(valueToString(false)).toBe('false');
  });

  it('keeps integers exact', () => {
    expect(valueToString(1920)).toBe('1920');
    expect(valueToString(0)).toBe('0');
  });

  it('trims trailing zeros from floats', () => {
    expect(valueToString(0.5)).toBe('0.5');
    expect(valueToString(1.25)).toBe('1.25');
    expect(valueToString(2.0)).toBe('2');
  });

  it('joins numeric arrays with comma + space and trims zeros', () => {
    expect(valueToString([1, 2, 3])).toBe('1, 2, 3');
    expect(valueToString([0.5, 0.25])).toBe('0.5, 0.25');
  });

  it('falls back to String() for plain strings', () => {
    expect(valueToString('hello')).toBe('hello');
  });
});
