import path from 'node:path';
import { readdirSync } from 'node:fs';

export function scanFolder(root: string, recursive: boolean, extSet: Set<string>): string[] {
  const found: string[] = [];
  function scan(dir: string): void {
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (recursive) scan(full);
      } else {
        const ext = path.extname(entry.name).slice(1).toLowerCase();
        if (extSet.has(ext)) found.push(full);
      }
    }
  }
  scan(root);
  return found;
}
