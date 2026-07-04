import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import type { NodeGraph, Progress } from '../../shared/types.js';
import { EXECUTOR, LIGHT_META_EXECUTORS } from '../../shared/constants.js';
import { topoSort, findOutputContributors } from './graph-utils.js';
import type { NodeRegistry } from '../nodes/registry.js';
import {
  buildCommandArgs,
  buildCommandArgsFromJs,
  buildFormatConvertArgs,
  getFormatExtension,
} from './command-builder.js';
import { getExecutor } from './executorRegistry.js';
import { loadImageMeta, buildEmptyImageMeta, type ImageMeta } from './executor-compute.js';
import { resolveNodeParams } from './resolve-params.js';
import { spawnMagick } from './magick-spawn.js';
import { log } from '../logger.js';
import { computeNewName, type RenameParams } from '../../shared/renameUtils.js';
import { timings } from './timing.js';
import { executeMultiStream, type BatchContext } from './multistream-pipeline.js';
import { executeSetBatch } from './set-pipeline.js';
import { executeTextBatch } from './text-output.js';
import { executeFlipbookBatch } from './atlas.js';

/**
 * Parse the compact IM7 verbose line into labeled fields.
 * Format: input=>output FORMAT WxH WxH+X+Y DEPTH colorspace... FILESIZE USERu ELAPSED
 */
function formatVerboseEntry(fileName: string, rawText: string): string {
  const lines = rawText
    .trim()
    .split('\n')
    .filter((l) => l.trim());
  const line = lines[0] ?? '';
  const arrowIdx = line.indexOf('=>');
  let out = `[verbose] ${fileName}\n`;

  if (arrowIdx !== -1) {
    const rest = line.slice(arrowIdx + 2).trim();
    const parts = rest.split(/\s+/);
    // parts: [outputPath, FORMAT, WxH, WxH+X+Y, DEPTH, ...colorspace, FILESIZE, USERu, ELAPSED]
    if (parts.length >= 9) {
      const [, format, canvas, , depth, ...middle] = parts;
      const elapsedRaw = middle[middle.length - 1] ?? '';
      const fileSize = middle[middle.length - 3] ?? '';
      const colorspace = middle.slice(0, middle.length - 3).join(' ');
      // Parse M:SS.mmm elapsed into plain seconds
      const [minPart, secPart] = elapsedRaw.split(':');
      const elapsedSec = (parseInt(minPart ?? '0', 10) * 60 + parseFloat(secPart ?? '0')).toFixed(3);
      out += `  format:      ${format}\n`;
      out += `  dimensions:  ${canvas}\n`;
      out += `  depth:       ${depth}\n`;
      if (colorspace) out += `  colorspace:  ${colorspace}\n`;
      out += `  file size:   ${fileSize.replace(/([0-9.]+)([A-Za-z]+)/, '$1 $2')}\n`;
      out += `  time:        ${elapsedSec} sec\n`;
    } else {
      out += lines.map((l) => '  ' + l).join('\n') + '\n';
    }
  } else {
    out += lines.map((l) => '  ' + l).join('\n') + '\n';
  }
  return out;
}

export async function executeBatch(
  graph: NodeGraph,
  outputNodeId: string,
  inputNodeId: string,
  imagePaths: string[],
  outputDir: string | null, // null = same directory as each source image
  overwrite: 'skip' | 'overwrite',
  registry: NodeRegistry,
  onProgress: (p: Progress) => void,
  isCancelled: () => boolean
): Promise<{ processed: number; skipped: number; failed: number; errors: string[]; outputFiles: string[] }> {
  // Text and flipbook output nodes have their own execution paths — the rest of
  // this function writes per-image image files, which only fits imageOutputNode.
  const outputNode = graph.nodes.find((n) => n.id === outputNodeId);
  if (outputNode?.type === 'textOutputNode') {
    return executeTextBatch(graph, outputNodeId, imagePaths, overwrite, registry, onProgress, isCancelled);
  }
  if (outputNode?.type === 'flipbookOutputNode') {
    return executeFlipbookBatch(graph, outputNodeId, imagePaths, overwrite);
  }

  const batchT0 = Date.now();
  log(
    'info',
    `[batch] start: ${imagePaths.length} image(s), output: ${outputDir ?? 'same as source'}, overwrite: ${overwrite}`
  );
  // Nothing to do — avoids concurrency=0 → os.cpus()/0 === Infinity downstream.
  if (imagePaths.length === 0) return { processed: 0, skipped: 0, failed: 0, errors: [], outputFiles: [] };
  const outputFiles: string[] = [];
  const batchVerboseEntries: string[] = [];
  const sorted = topoSort(graph.nodes, graph.edges);

  // Whether the workflow produces an image output (edge to outputNodeId in-0).
  const hasImageOutput = graph.edges.some((e) => e.target === outputNodeId && e.targetHandle === 'in-0');

  // Nodes that actually contribute to the final output — backward BFS from
  // outputNodeId following ALL edges (image AND param-wire).
  // This ensures channel_split that feeds mean_value → gate (via param-wires)
  // is correctly recognised as a contributor and uses the multi-stream path.
  // Nodes that are purely decorative (no path to outputNodeId of any kind)
  // are excluded so they don't force the slow path unnecessarily.
  const outputContributorIds = findOutputContributors(graph.edges, [outputNodeId]);

  // prop_ nodes depend on per-image file metadata (dimensions, name, EXIF, etc.)
  // mean_value depends on per-image pixel data but NOT on loadImageMeta.
  // Either kind requires per-image plan evaluation (no shared opArgs).
  // Only nodes that contribute to output are relevant — disconnected nodes must not
  // influence the shared-plan decision or trigger unnecessary metadata calls.
  const hasImageMetaNodes = sorted.some((n) => {
    if (!outputContributorIds.has(n.id)) return false;
    const def = registry.get(n.data.definitionId);
    return def?.needs_image_meta === true;
  });
  const hasHeavyMetaNodes = sorted.some((n) => {
    if (!outputContributorIds.has(n.id)) return false;
    const def = registry.get(n.data.definitionId);
    return def?.needs_image_meta === true && !LIGHT_META_EXECUTORS.has(def.executor ?? '');
  });
  const hasPropNodes =
    hasImageMetaNodes ||
    sorted.some((n) => {
      if (!outputContributorIds.has(n.id)) return false;
      const def = registry.get(n.data.definitionId);
      return def?.executor === EXECUTOR.MEAN_VALUE;
    });

  // Executors that require executeMultiStream (cannot be handled by the fast-path
  // buildOpArgsForImage): channel_split/merge produce multiple image buffers;
  // mean_value reads pixel data per-image and feeds param-wires (gate conditions etc.)
  // — buildOpArgsForImage skips it, leaving downstream gate conditions unset.
  const MULTI_STREAM_EXECUTORS = new Set<string>([EXECUTOR.CHANNEL_SPLIT, EXECUTOR.CHANNEL_MERGE, EXECUTOR.MEAN_VALUE]);
  const hasMultiStreamNodes = sorted.some((n) => {
    if (!outputContributorIds.has(n.id)) return false;
    const def = registry.get(n.data.definitionId);
    return def?.executor && MULTI_STREAM_EXECUTORS.has(def.executor);
  });

  interface BatchPlan {
    opArgs: string[];
    outputFormat: string | null; // e.g. 'PNG' — non-null only when format_convert is active
  }

  // Returns null when a Gate node suppresses the image (don't write output).
  async function buildOpArgsForImage(imagePath: string): Promise<BatchPlan | null> {
    let meta: ImageMeta | undefined;
    if (hasHeavyMetaNodes) {
      try {
        meta = await loadImageMeta(imagePath);
      } catch (err) {
        console.warn(`[executor] loadImageMeta failed for ${imagePath} (non-fatal, prop nodes use defaults):`, err);
      }
    } else if (hasImageMetaNodes && imagePath !== '') {
      // light-meta nodes only — no ImageMagick spawn needed.
      meta = await buildEmptyImageMeta(imagePath);
    }
    const resolvedParams = new Map<string, Record<string, unknown>>();
    const opArgs: string[] = [];
    let outputFormat: string | null = null;
    for (const node of sorted) {
      if (!outputContributorIds.has(node.id)) continue;
      if (node.data.definitionId === EXECUTOR.PROCESS_AS_SET) continue;
      const def = registry.get(node.data.definitionId);
      if (!def) continue;
      const { params, isImageNode } = resolveNodeParams(node, def, graph.edges, resolvedParams, meta);
      if (!isImageNode) continue;
      // Gate node: when active and condition is false, suppress this image entirely
      if (def.executor === EXECUTOR.GATE && params._enabled !== false && !params.condition) return null;
      // Mean Value — analysis-only, no image output, no opArgs contribution
      if (def.executor === EXECUTOR.MEAN_VALUE) continue;
      if (params._enabled !== false) {
        if (def.executor === EXECUTOR.FORMAT_CONVERT) {
          outputFormat = String(params.format ?? 'PNG').toUpperCase();
          opArgs.push(...buildFormatConvertArgs(outputFormat, params));
        } else {
          const registeredFn = def.executor ? getExecutor(def.executor) : undefined;
          opArgs.push(
            ...(registeredFn
              ? registeredFn(def, params)
              : def.command_js
                ? buildCommandArgsFromJs(def, params)
                : buildCommandArgs(def, params))
          );
        }
      }
    }
    return { opArgs, outputFormat };
  }

  // Fast path: no Properties nodes — evaluate once, reuse for all images.
  // undefined = not pre-computed (will be built per-image); null = gate suppressed for all images.
  let sharedPlan: BatchPlan | null | undefined;
  if (!hasPropNodes && !hasMultiStreamNodes) {
    try {
      sharedPlan = await buildOpArgsForImage('');
    } catch (err) {
      throw Object.assign(new Error(`Pipeline build failed: ${err instanceof Error ? err.message : String(err)}`), {
        cause: err,
      });
    }
  }

  // Build the shared context object passed to executeMultiStream and executeSetBatch.
  const ctx: BatchContext = {
    inputNodeId,
    outputNodeId,
    sorted,
    outputContributorIds,
    registry,
    graph,
    hasHeavyMetaNodes,
    hasImageMetaNodes,
    hasImageOutput,
    outputDir,
    overwrite,
    isCancelled,
    spawnEnv: process.env, // overridden below once this run's concurrency is known
  };

  // ── Set batch mode ─────────────────────────────────────────────────────────
  // When a setInputNode is present, group images by naming convention and
  // execute one run per set instead of one run per image.
  const setInputNode = sorted.find((n) => n.data.definitionId === EXECUTOR.PROCESS_AS_SET);
  if (setInputNode) {
    return executeSetBatch(ctx, setInputNode, imagePaths, batchT0, onProgress);
  }

  // Run all images concurrently, capped at 128 for very large batches.
  // With MAGICK_THREAD_LIMIT=1, each process uses exactly one thread so small
  // oversubscription (e.g. 26 images on 24 cores) costs less than a full extra round.
  // JS single-threaded event loop guarantees queueIdx++ is race-free.
  const concurrency = Math.min(128, imagePaths.length);
  let queueIdx = 0;
  let completed = 0;
  let failures = 0;
  let skipped = 0;
  const errors: string[] = [];

  if (timings.enabled) {
    timings.startBatch(imagePaths.length);
    timings.recordSetup(Date.now() - batchT0);
  }

  const activeImages = new Set<string>();
  onProgress({ completed: 0, total: imagePaths.length, currentFile: '', active: [] });

  // Resolve rename node params once (shared across all images — index varies per image)
  const renameNode = sorted.find((n) => registry.get(n.data.definitionId)?.executor === EXECUTOR.RENAME);
  const renameParams = renameNode ? (renameNode.data.params as RenameParams) : undefined;

  let _startupRecorded = false;

  async function processOne(): Promise<void> {
    while (queueIdx < imagePaths.length) {
      if (isCancelled()) return;
      const imageIndex = queueIdx; // 0-based index for rename numbering
      const inputPath = imagePaths[queueIdx++];
      const fileName = path.basename(inputPath);
      log('info', `[batch] processing (${imageIndex + 1}/${imagePaths.length}): ${fileName}`);
      activeImages.add(fileName);
      onProgress({ completed, total: imagePaths.length, currentFile: fileName, active: [...activeImages] });
      const imgT0 = Date.now();
      if (timings.enabled && !_startupRecorded) {
        _startupRecorded = true;
        timings.recordStartup(Date.now() - batchT0);
      }
      const magickBucket = timings.enabled
        ? {
            ms: 0,
            add(n: number) {
              this.ms += n;
            },
          }
        : null;
      // Apply rename transform to determine the output filename stem
      const renamedFileName = renameParams ? computeNewName(fileName, renameParams, imageIndex) : fileName;
      try {
        if (hasMultiStreamNodes) {
          // Multi-stream path — runs concurrently; unique tmpId per image prevents collisions
          const targetDir = outputDir ?? path.dirname(inputPath);
          const checkT0 = timings.enabled ? Date.now() : 0;
          await fs.promises.mkdir(targetDir, { recursive: true });
          let fileCheckMs = timings.enabled ? Date.now() - checkT0 : 0;
          const msT0 = timings.enabled ? Date.now() : 0;
          const msVerboseCapture: string[] = [];
          const msResult = await executeMultiStream(
            inputPath,
            imageIndex,
            ctx,
            undefined,
            timings.enabled ? msVerboseCapture : undefined
          );
          const msElapsed = timings.enabled ? Date.now() - msT0 : 0;
          if (msResult === null) {
            skipped++;
            log('info', `[batch] skipped (gate): ${fileName}`);
            activeImages.delete(fileName);
            onProgress({
              completed: ++completed,
              total: imagePaths.length,
              currentFile: fileName,
              active: [...activeImages],
            });
            continue;
          }
          if (hasImageOutput) {
            const { resultPath, outputExt: msExt, cleanup } = msResult;
            const outExt = msExt || path.extname(renamedFileName);
            const outBase = path.basename(renamedFileName, path.extname(renamedFileName));
            const outPath = path.join(targetDir, outBase + outExt);

            if (overwrite === 'skip') {
              const accessT0 = timings.enabled ? Date.now() : 0;
              const exists = await fs.promises
                .access(outPath)
                .then(() => true)
                .catch(() => false);
              if (timings.enabled) fileCheckMs += Date.now() - accessT0;
              if (exists) {
                skipped++;
                log('info', `[batch] skipped (exists): ${fileName}`);
                activeImages.delete(fileName);
                onProgress({
                  completed: ++completed,
                  total: imagePaths.length,
                  currentFile: fileName,
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
              const imgEntry = timings.beginImage(inputPath);
              imgEntry.fileCheck(fileCheckMs);
              imgEntry.magick(msElapsed);
              imgEntry.copy(Date.now() - copyT0);
              imgEntry.done(Date.now() - imgT0);
            }
            log('info', `[batch] done (${Date.now() - imgT0}ms): ${fileName} → ${outPath}`);
            outputFiles.push(outPath);
            if (timings.enabled && msVerboseCapture.length > 0) {
              const verboseText = msVerboseCapture.join('').trim();
              if (verboseText) batchVerboseEntries.push(formatVerboseEntry(fileName, verboseText));
            }
          } else {
            void msResult.cleanup();
          }
        } else {
          // Single-command fast path — evaluate the plan before creating the output directory
          // so gate-suppressed images are skipped without attempting mkdir on a non-existent path.
          const plan = sharedPlan !== undefined ? sharedPlan : await buildOpArgsForImage(inputPath);
          if (plan === null) {
            skipped++;
            log('info', `[batch] skipped (gate): ${fileName}`);
            activeImages.delete(fileName);
            onProgress({
              completed: ++completed,
              total: imagePaths.length,
              currentFile: fileName,
              active: [...activeImages],
            });
            continue;
          }
          if (hasImageOutput) {
            const targetDir = outputDir ?? path.dirname(inputPath);
            const checkT0 = timings.enabled ? Date.now() : 0;
            await fs.promises.mkdir(targetDir, { recursive: true });
            let fileCheckMs = timings.enabled ? Date.now() - checkT0 : 0;
            const { opArgs, outputFormat } = plan;
            const outExt = outputFormat ? getFormatExtension(outputFormat) : path.extname(renamedFileName);
            const outBase = path.basename(renamedFileName, path.extname(renamedFileName));
            const outPath = path.join(targetDir, outBase + outExt);

            if (overwrite === 'skip') {
              const accessT0 = timings.enabled ? Date.now() : 0;
              const exists = await fs.promises
                .access(outPath)
                .then(() => true)
                .catch(() => false);
              if (timings.enabled) fileCheckMs += Date.now() - accessT0;
              if (exists) {
                skipped++;
                log('info', `[batch] skipped (exists): ${fileName}`);
                activeImages.delete(fileName);
                onProgress({
                  completed: ++completed,
                  total: imagePaths.length,
                  currentFile: fileName,
                  active: [...activeImages],
                });
                continue;
              }
            }

            if (opArgs.length > 0 || outputFormat) {
              const fmtOut = outputFormat ? `${outputFormat}:${outPath}` : outPath;
              const verboseCapture: string[] = [];
              const verboseArgs = timings.enabled ? ['-verbose'] : [];
              await spawnMagick(
                [...verboseArgs, inputPath, ...opArgs, fmtOut],
                magickBucket ?? undefined,
                undefined,
                timings.enabled ? verboseCapture : undefined,
                { env: ctx.spawnEnv }
              );
              if (timings.enabled && verboseCapture.length > 0) {
                const verboseText = verboseCapture.join('').trim();
                if (verboseText) batchVerboseEntries.push(formatVerboseEntry(fileName, verboseText));
              }
              if (timings.enabled && magickBucket) {
                const imgEntry = timings.beginImage(inputPath);
                imgEntry.fileCheck(fileCheckMs);
                imgEntry.magick(magickBucket.ms);
                imgEntry.copy(0);
                imgEntry.done(Date.now() - imgT0);
              }
            } else {
              const copyT0 = timings.enabled ? Date.now() : 0;
              await fs.promises.copyFile(inputPath, outPath);
              if (timings.enabled && magickBucket) {
                const imgEntry = timings.beginImage(inputPath);
                imgEntry.fileCheck(fileCheckMs);
                imgEntry.magick(0);
                imgEntry.copy(Date.now() - copyT0);
                imgEntry.done(Date.now() - imgT0);
              }
            }
            log('info', `[batch] done (${Date.now() - imgT0}ms): ${fileName} → ${outPath}`);
            outputFiles.push(outPath);
          }
        }
      } catch (err) {
        failures++;
        const msg = err instanceof Error ? err.message : String(err);
        errors.push(`${fileName}: ${msg}`);
        console.error(`[executor] Failed to process ${fileName}:`, err);
      }
      activeImages.delete(fileName);
      onProgress({
        completed: ++completed,
        total: imagePaths.length,
        currentFile: fileName,
        active: [...activeImages],
      });
    }
  }

  // Prevent magick's internal OpenMP thread pool from oversubscribing the CPU.
  // With N concurrent pipelines each trying to use all cores, we'd get N×cores threads
  // competing for cores threads — massive context-switching overhead.  Giving each
  // process an equal share of the hardware threads keeps total thread count at os.cpus().
  const threadsPerProcess = Math.max(1, Math.floor(os.cpus().length / concurrency));
  // Pass the thread limit per-spawn (not via process.env) so concurrent previews,
  // thumbnail generation, or a second output node don't inherit this throttle and
  // overlapping batches can't clobber each other's save/restore.
  ctx.spawnEnv = { ...process.env, MAGICK_THREAD_LIMIT: String(threadsPerProcess) };
  await Promise.all(Array.from({ length: concurrency }, processOne));
  if (timings.enabled) {
    const resolvedOutputDir = outputDir ?? (outputFiles.length > 0 ? path.dirname(outputFiles[0]) : null);
    timings.endBatch(resolvedOutputDir, batchVerboseEntries.length > 0 ? batchVerboseEntries : undefined);
  }
  log(
    'info',
    `[batch] complete: ${completed - failures - skipped} processed, ${skipped} skipped, ${failures} failed in ${Date.now() - batchT0}ms`
  );
  return { processed: completed - failures - skipped, skipped, failed: failures, errors, outputFiles };
}
