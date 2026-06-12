import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Regression guard for the "Blocked IPC on unknown channel" class of bug.
//
// The preload allowlist (electron/preload.ts) is built from Object.values(IPC).
// Any channel passed to ipcRenderer.on/off/send/invoke or webContents.send that
// is NOT a literal IPC.* value is rejected at runtime — which silently aborts
// the renderer mount. The original break was an inline `${IPC.EXECUTE_BATCH}:progress`
// channel that, being derived, never appeared in the allowlist.
//
// Rule: IPC channel names must be declared constants in `IPC`, never built by
// interpolating an existing IPC.* value. If you need a sub-channel, add a real
// constant for it (e.g. EXECUTE_BATCH_PROGRESS).

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const scanDirs = ['src', 'electron'];
const sourceExt = new Set(['.ts', '.svelte']);

function collectSourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = path.join(dir, entry);
    if (statSync(full).isDirectory()) {
      if (entry === 'node_modules' || entry === 'tests') continue;
      out.push(...collectSourceFiles(full));
    } else if (sourceExt.has(path.extname(entry))) {
      out.push(full);
    }
  }
  return out;
}

describe('IPC channel hygiene', () => {
  it('never constructs a channel by interpolating an IPC.* value', () => {
    // Matches `${IPC.SOMETHING}` inside a template literal — a derived channel
    // name that bypasses the preload allowlist.
    const dynamicChannel = /\$\{\s*IPC\./;
    const offenders: string[] = [];

    for (const dir of scanDirs) {
      for (const file of collectSourceFiles(path.join(repoRoot, dir))) {
        const lines = readFileSync(file, 'utf8').split('\n');
        lines.forEach((line, i) => {
          if (dynamicChannel.test(line)) {
            offenders.push(`${path.relative(repoRoot, file)}:${i + 1}: ${line.trim()}`);
          }
        });
      }
    }

    expect(
      offenders,
      `IPC channels must be declared constants, not interpolated from IPC.*:\n${offenders.join('\n')}`
    ).toEqual([]);
  });
});
