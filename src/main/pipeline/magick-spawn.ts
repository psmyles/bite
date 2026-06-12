import { spawn, type StdioOptions } from 'node:child_process';
import { getMagickBinary } from './magick-path.js';
import { log } from '../logger.js';

export interface SpawnMagickOptions {
  env?: NodeJS.ProcessEnv;
  stdio?: StdioOptions;
}

export function spawnMagick(
  args: string[],
  bucket?: { add(ms: number): void },
  timeoutMs?: number,
  verboseOut?: string[],
  opts?: SpawnMagickOptions
): Promise<void> {
  const t0 = Date.now();
  const label = `${args[0] ?? ''}${args.length > 1 ? ` → ${args[args.length - 1]}` : ''} (${args.length} args)`;
  log('info', `[magick] spawn: ${label}`);
  return new Promise((resolve, reject) => {
    const proc = spawn(getMagickBinary(), args, { env: opts?.env, stdio: opts?.stdio });
    const stderr: string[] = [];
    const stdout: string[] = [];

    // Guard against the timing bucket being added twice: on timeout we kill the
    // process, which then fires 'close' — both paths would otherwise record time.
    let settled = false;
    const settle = () => {
      if (settled) return false;
      settled = true;
      if (bucket) bucket.add(Date.now() - t0);
      return true;
    };

    let timer: ReturnType<typeof setTimeout> | undefined;
    if (timeoutMs !== undefined) {
      timer = setTimeout(() => {
        proc.kill();
        if (!settle()) return;
        log('warn', `[magick] timed out after ${timeoutMs}ms: ${label}`);
        reject(new Error(`magick timed out after ${timeoutMs}ms`));
      }, timeoutMs);
    }

    proc.stdout?.on('data', (chunk: Buffer) => stdout.push(chunk.toString()));
    proc.stderr?.on('data', (chunk: Buffer) => stderr.push(chunk.toString()));
    proc.on('close', (code) => {
      clearTimeout(timer);
      if (!settle()) return;
      if (code === 0) {
        log('info', `[magick] done in ${Date.now() - t0}ms: ${label}`);
        if (verboseOut) {
          const capturedStdout = stdout.join('');
          const capturedStderr = stderr.join('');
          log('info', `[magick] verbose capture: stdout=${capturedStdout.length}B stderr=${capturedStderr.length}B`);
          verboseOut.push(capturedStdout + capturedStderr);
        }
        resolve();
      } else {
        log('error', `[magick] exited ${code} in ${Date.now() - t0}ms: ${stderr.join('').trim().slice(0, 300)}`);
        reject(new Error(`magick exited ${code}: ${stderr.join('').trim()}`));
      }
    });
    proc.on('error', (err) => {
      clearTimeout(timer);
      if (!settle()) return;
      log('error', `[magick] spawn failed: ${err.message}`);
      reject(new Error(`Failed to spawn magick: ${err.message}. Is ImageMagick v7 in PATH?`));
    });
  });
}

export function spawnMagickCapture(args: string[], timeoutMs?: number, opts?: SpawnMagickOptions): Promise<string> {
  const t0 = Date.now();
  const label = `${args[0] ?? ''}${args.length > 1 ? ` → ${args[args.length - 1]}` : ''} (${args.length} args)`;
  log('info', `[magick] spawn (capture): ${label}`);
  return new Promise((resolve, reject) => {
    const proc = spawn(getMagickBinary(), args, { env: opts?.env, stdio: opts?.stdio });
    const out: string[] = [];
    const err: string[] = [];
    let timer: ReturnType<typeof setTimeout> | undefined;
    if (timeoutMs !== undefined) {
      timer = setTimeout(() => {
        proc.kill();
        log('warn', `[magick] timed out after ${timeoutMs}ms: ${label}`);
        reject(new Error(`magick timed out after ${timeoutMs}ms`));
      }, timeoutMs);
    }
    proc.stdout?.on('data', (chunk: Buffer) => out.push(chunk.toString()));
    proc.stderr?.on('data', (chunk: Buffer) => err.push(chunk.toString()));
    proc.on('close', (code) => {
      clearTimeout(timer);
      if (code === 0) {
        log('info', `[magick] done in ${Date.now() - t0}ms: ${label}`);
        resolve(out.join(''));
      } else {
        log('error', `[magick] exited ${code} in ${Date.now() - t0}ms: ${err.join('').trim().slice(0, 300)}`);
        reject(new Error(`magick exited ${code}: ${err.join('').trim()}`));
      }
    });
    proc.on('error', (e) => {
      clearTimeout(timer);
      log('error', `[magick] spawn failed: ${e.message}`);
      reject(new Error(`Failed to spawn magick: ${e.message}. Is ImageMagick v7 in PATH?`));
    });
  });
}
