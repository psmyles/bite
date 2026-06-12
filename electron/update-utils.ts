// Pure helpers for the Electron bootstrap (no electron imports) so they can be
// unit-tested without booting the app.

/**
 * Find a `.imgplex` file path among process argv.
 * - Packaged:    argv = ['imgplex.exe', ...args]      → start at index 1
 * - Development: argv = ['electron', 'main.js', ...]   → start at index 2
 */
export function extractImgplexPath(argv: string[], isPackaged: boolean): string | null {
  const start = isPackaged ? 1 : 2;
  for (let i = start; i < argv.length; i++) {
    if (!argv[i].startsWith('-') && argv[i].endsWith('.imgplex')) return argv[i];
  }
  return null;
}

/** Compare two semver strings. Returns >0 if a is newer, <0 if older, 0 if equal. */
export function compareSemver(a: string, b: string): number {
  // Strip a leading 'v' and any pre-release/build suffix (e.g. '1.2.3-beta') so the
  // numeric parts don't become NaN.
  const parse = (v: string) =>
    v
      .replace(/^v/, '')
      .split('.')
      .map((part) => parseInt(part, 10) || 0);
  const [aMaj, aMin, aPat] = parse(a);
  const [bMaj, bMin, bPat] = parse(b);
  if (aMaj !== bMaj) return aMaj - bMaj;
  if (aMin !== bMin) return aMin - bMin;
  return aPat - bPat;
}
