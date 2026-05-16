import fs from 'node:fs';
import path from 'node:path';

export interface LogEntry { ts: string; level: 'info' | 'warn' | 'error'; msg: string }

interface LogWindow {
  isDestroyed(): boolean;
  webContents: { send(channel: string, ...args: unknown[]): void };
}

const entries: LogEntry[] = [];
let logStream: fs.WriteStream | null = null;
let logWin: LogWindow | null = null;

export function initLogger(logDir: string): void {
  fs.mkdirSync(logDir, { recursive: true });
  logStream = fs.createWriteStream(path.join(logDir, 'imgplex.log'), { flags: 'a' });
  logStream.write(`\n--- Session ${new Date().toISOString()} ---\n`);
}

export function setLogWindow(win: LogWindow | null): void { logWin = win; }
export function getEntries(): LogEntry[] { return entries; }

export function log(level: LogEntry['level'], ...args: unknown[]): void {
  const ts = new Date().toISOString();
  const msg = args
    .map((a) => a instanceof Error ? `${a.message}\n${a.stack ?? ''}` : typeof a === 'object' ? JSON.stringify(a) : String(a))
    .join(' ');
  const entry: LogEntry = { ts, level, msg };
  entries.push(entry);
  logStream?.write(`[${ts}] [${level.toUpperCase()}] ${msg}\n`);
  if (logWin && !logWin.isDestroyed()) logWin.webContents.send('log:entry', entry);
}
