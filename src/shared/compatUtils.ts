import ranges from './compat-ranges.json';

interface CompatRange {
  from: string;
  to: string | null;
}

function parseVer(v: string): [number, number, number] | null {
  const parts = v.split('.').map(Number);
  if (parts.length !== 3 || parts.some(isNaN)) return null;
  return parts as [number, number, number];
}

function cmp(a: [number, number, number], b: [number, number, number]): number {
  return a[0] - b[0] || a[1] - b[1] || a[2] - b[2];
}

function rangeIndex(version: string): number {
  const v = parseVer(version);
  if (!v) return -1;
  return (ranges as CompatRange[]).findIndex((r) => {
    const from = parseVer(r.from);
    if (!from || cmp(v, from) < 0) return false;
    if (r.to === null) return true;
    const to = parseVer(r.to);
    if (!to) return false;
    return cmp(v, to) <= 0;
  });
}

export function isCompatible(fileVersion: string, appVersion: string): boolean {
  const fi = rangeIndex(fileVersion);
  const ai = rangeIndex(appVersion);
  if (fi === -1 || ai === -1) return false;
  return fi === ai;
}
