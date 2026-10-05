import path from 'node:path';
import fs from 'node:fs';

export async function scanFolder(root: string, recursive: boolean, extSet: Set<string>): Promise<string[]> {
  const found: string[] = [];
  async function scan(dir: string): Promise<void> {
    let entries: import('node:fs').Dirent[];
    try {
      entries = await fs.promises.readdir(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (recursive) await scan(full);
      } else {
        const ext = path.extname(entry.name).slice(1).toLowerCase();
        if (extSet.has(ext)) found.push(full);
      }
    }
  }
  await scan(root);
  return found;
}
