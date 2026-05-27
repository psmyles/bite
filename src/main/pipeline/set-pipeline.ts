import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import type { GraphNode, Progress } from '../../shared/types.js';
import { groupBySetPattern } from './graph-utils.js';
import { timings } from './timing.js';
import { log } from '../logger.js';
import { executeMultiStream, type BatchContext } from './multistream-pipeline.js';

export async function executeSetBatch(
  ctx: BatchContext,
  setInputNode: GraphNode,
  imagePaths: string[],
  batchT0: number,
  onProgress: (p: Progress) => void
): Promise<{ processed: number; skipped: number; failed: number; errors: string[]; outputFiles: string[] }> {
  const { outputDir, overwrite, isCancelled, hasImageOutput } = ctx;
  const outputNode = ctx.sorted.find((n) => n.id === ctx.outputNodeId);

  const setPrefix = String(setInputNode.data.params?.prefix ?? '');
  const setSuffixes = Array.isArray(setInputNode.data.params?.suffixes)
    ? (setInputNode.data.params!.suffixes as string[]).filter(Boolean)
    : [];
  const outParams = outputNode?.data.params as Record<string, unknown> | undefined;
  const setOutputPrefix = String(outParams?.setOutputPrefix ?? '');
  const setOutputSuffix = String(outParams?.setOutputSuffix ?? '');

  const setGroups = groupBySetPattern(imagePaths, setPrefix, setSuffixes);
  const setEntries = [...setGroups.entries()];
  const totalSets = setEntries.length;

  if (timings.enabled) {
    timings.startBatch(totalSets);
    timings.recordSetup(Date.now() - batchT0);
  }

  let setQueueIdx = 0;
  let setCompleted = 0;
  let setFailures = 0;
  let setSkipped = 0;
  const setErrors: string[] = [];
  const outputFiles: string[] = [];
  let _setStartupRecorded = false;
  const activeImages = new Set<string>();

  onProgress({ completed: 0, total: totalSets, currentFile: '', active: [] });

  const setConcurrency = Math.min(Math.max(1, os.cpus().length), Math.max(1, totalSets));

  async function processOneSet(): Promise<void> {
    while (setQueueIdx < setEntries.length) {
      if (isCancelled()) return;
      const setIndex = setQueueIdx;
      const [middleName, suffixMap] = setEntries[setQueueIdx++];
      log('info', `[batch] processing set (${setQueueIdx}/${totalSets}): ${middleName}`);
      activeImages.add(middleName);
      onProgress({ completed: setCompleted, total: totalSets, currentFile: middleName, active: [...activeImages] });
      const imgT0 = Date.now();
      if (timings.enabled && !_setStartupRecorded) {
        _setStartupRecorded = true;
        timings.recordStartup(Date.now() - batchT0);
      }

      // First available path serves as the inputPath fallback for unconnected ports.
      const firstPath = setSuffixes.map((s) => suffixMap[s]).find(Boolean) ?? '';

      // Pre-seed one buffer per suffix port; missing suffixes are left unseeded
      // so their downstream nodes receive the firstPath fallback.
      const seeds = new Map<string, string>();
      const setNodeId = setInputNode.id;
      for (let i = 0; i < setSuffixes.length; i++) {
        const p = suffixMap[setSuffixes[i]];
        if (p) seeds.set(`${setNodeId}:out-${i}`, p);
      }

      try {
        const targetDir = outputDir ?? (firstPath ? path.dirname(firstPath) : process.cwd());
        const checkT0 = timings.enabled ? Date.now() : 0;
        await fs.promises.mkdir(targetDir, { recursive: true });
        let fileCheckMs = timings.enabled ? Date.now() - checkT0 : 0;

        if (hasImageOutput) {
          const msT0 = timings.enabled ? Date.now() : 0;
          const msResult = await executeMultiStream(firstPath, setIndex, ctx, seeds);
          const msElapsed = timings.enabled ? Date.now() - msT0 : 0;
          if (msResult === null) {
            setSkipped++;
            log('info', `[batch] skipped set (gate): ${middleName}`);
            activeImages.delete(middleName);
            onProgress({
              completed: ++setCompleted,
              total: totalSets,
              currentFile: middleName,
              active: [...activeImages],
            });
            continue;
          }
          const { resultPath, outputExt: msExt, cleanup } = msResult;
          const outBase = setOutputPrefix + middleName + setOutputSuffix;
          const outExt = msExt || path.extname(firstPath);
          const outPath = path.join(targetDir, outBase + outExt);

          if (overwrite === 'skip') {
            const accessT0 = timings.enabled ? Date.now() : 0;
            const exists = await fs.promises
              .access(outPath)
              .then(() => true)
              .catch(() => false);
            if (timings.enabled) fileCheckMs += Date.now() - accessT0;
            if (exists) {
              setSkipped++;
              log('info', `[batch] skipped set (exists): ${middleName}`);
              activeImages.delete(middleName);
              onProgress({
                completed: ++setCompleted,
                total: totalSets,
                currentFile: middleName,
                active: [...activeImages],
              });
              void cleanup();
              continue;
            }
          }
          const copyT0 = timings.enabled ? Date.now() : 0;
          await fs.promises.copyFile(resultPath, outPath);
          void cleanup();
          if (timings.enabled) {
            const imgEntry = timings.beginImage(firstPath || middleName);
            imgEntry.fileCheck(fileCheckMs);
            imgEntry.magick(msElapsed);
            imgEntry.copy(Date.now() - copyT0);
            imgEntry.done(Date.now() - imgT0);
          }
          log('info', `[batch] done set (${Date.now() - imgT0}ms): ${middleName} → ${outPath}`);
          outputFiles.push(outPath);
        }
      } catch (err) {
        setFailures++;
        const msg = err instanceof Error ? err.message : String(err);
        setErrors.push(`${middleName}: ${msg}`);
        console.error(`[executor] Failed to process set "${middleName}":`, err);
      }
      activeImages.delete(middleName);
      onProgress({ completed: ++setCompleted, total: totalSets, currentFile: middleName, active: [...activeImages] });
    }
  }

  const threadsPerSetProcess = Math.max(1, Math.floor(os.cpus().length / setConcurrency));
  const prevThreadLimitSet = process.env.MAGICK_THREAD_LIMIT;
  process.env.MAGICK_THREAD_LIMIT = String(threadsPerSetProcess);
  try {
    await Promise.all(Array.from({ length: setConcurrency }, processOneSet));
  } finally {
    if (prevThreadLimitSet !== undefined) process.env.MAGICK_THREAD_LIMIT = prevThreadLimitSet;
    else delete process.env.MAGICK_THREAD_LIMIT;
  }
  if (timings.enabled) {
    const resolvedOutputDir = outputDir ?? (outputFiles.length > 0 ? path.dirname(outputFiles[0]) : null);
    timings.endBatch(resolvedOutputDir);
  }
  log(
    'info',
    `[batch] complete (set mode): ${setCompleted - setFailures - setSkipped} processed, ${setSkipped} skipped, ${setFailures} failed in ${Date.now() - batchT0}ms`
  );
  return {
    processed: setCompleted - setFailures - setSkipped,
    skipped: setSkipped,
    failed: setFailures,
    errors: setErrors,
    outputFiles,
  };
}
