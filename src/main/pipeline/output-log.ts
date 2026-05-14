import fs from 'node:fs';
import path from 'node:path';

export async function writeOutputLog(opts: {
  outputFiles: string[];
  durationMs: number;
  outputDir: string | null;
}): Promise<void> {
  if (opts.outputFiles.length === 0) return;
  const now = new Date();
  const yy = String(now.getFullYear()).slice(2);
  const mm = String(now.getMonth() + 1).padStart(2, '0');
  const dd = String(now.getDate()).padStart(2, '0');
  const hh = String(now.getHours()).padStart(2, '0');
  const mn = String(now.getMinutes()).padStart(2, '0');
  const logName = `outputlog_${yy}${mm}${dd}_${hh}${mn}.log`;

  const totalSec = opts.durationMs / 1000;
  const mins = Math.floor(totalSec / 60);
  const secs = totalSec % 60;
  const durationStr = mins > 0 ? `${mins}m ${secs.toFixed(0)}s` : `${secs.toFixed(2)}s`;
  const avgStr = (totalSec / opts.outputFiles.length).toFixed(2) + 's';
  const folderStr = opts.outputDir ?? 'Same as source';
  const useFullPaths = !opts.outputDir;
  const fileLines = opts.outputFiles.map((f) => (useFullPaths ? f : path.basename(f)));

  const content =
    [
      'imgplex Output Log',
      `Generated: ${now.getFullYear()}-${mm}-${dd} ${hh}:${mn}`,
      '',
      `Duration:         ${durationStr}`,
      `Files output:     ${opts.outputFiles.length}`,
      `Avg per file:     ${avgStr}`,
      `Output folder:    ${folderStr}`,
      '',
      'Files:',
      ...fileLines,
    ].join('\n') + '\n';

  const logDir = opts.outputDir ?? path.dirname(opts.outputFiles[0]);
  await fs.promises.mkdir(logDir, { recursive: true });
  await fs.promises.writeFile(path.join(logDir, logName), content, 'utf-8');
}
