<script lang="ts">
  import type { Node } from '@xyflow/svelte';
  import { graphStore } from '../stores/graph.svelte.js';
  import { imageStore } from '../stores/images.svelte.js';
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js';

  let { selectedNode }: { selectedNode: Node } = $props();

  const params = $derived(getNodeParams(selectedNode?.data));
  const prefix = $derived(String(params.prefix ?? ''));
  const suffixes = $derived(Array.isArray(params.suffixes) ? (params.suffixes as string[]) : []);

  // ── Suffix editing ────────────────────────────────────────────────────────

  function setPrefix(val: string) {
    graphStore.setParam(selectedNode.id, 'prefix', val);
  }

  function setSuffixes(next: string[]) {
    graphStore.setParam(selectedNode.id, 'suffixes', next);
  }

  function addSuffix() {
    setSuffixes([...suffixes, '']);
  }

  function removeSuffix(i: number) {
    setSuffixes(suffixes.filter((_, idx) => idx !== i));
  }

  function updateSuffix(i: number, val: string) {
    const next = [...suffixes];
    next[i] = val;
    setSuffixes(next);
  }

  // ── Wired prefix/suffix detection ────────────────────────────────────────

  function isPrefixWired(): boolean {
    return graphStore.edges.some((e) => e.target === selectedNode.id && e.targetHandle === 'prefix-in');
  }

  function getWiredPrefixValue(): string {
    const edge = graphStore.edges.find((e) => e.target === selectedNode.id && e.targetHandle === 'prefix-in');
    if (!edge) return prefix;
    const srcNode = graphStore.nodes.find((n) => n.id === edge.source);
    if (!srcNode) return prefix;
    const srcParamName = (edge.sourceHandle ?? '').replace('param-out-', '');
    return String(getNodeParams(srcNode.data)[srcParamName] ?? prefix);
  }

  function isSuffixWired(i: number): boolean {
    return graphStore.edges.some((e) => e.target === selectedNode.id && e.targetHandle === `suf-in-${i}`);
  }

  function getWiredSuffixValue(i: number): string {
    const edge = graphStore.edges.find((e) => e.target === selectedNode.id && e.targetHandle === `suf-in-${i}`);
    if (!edge) return suffixes[i] ?? '';
    const srcNode = graphStore.nodes.find((n) => n.id === edge.source);
    if (!srcNode) return suffixes[i] ?? '';
    const srcParamName = (edge.sourceHandle ?? '').replace('param-out-', '');
    const srcParams = getNodeParams(srcNode.data);
    return String(srcParams[srcParamName] ?? suffixes[i] ?? '');
  }

  // ── Live set preview ──────────────────────────────────────────────────────

  interface SetGroup {
    middle: string;
    slots: Record<string, string | undefined>;
    complete: boolean;
  }

  const setGroups = $derived.by((): SetGroup[] => {
    if (suffixes.length === 0) return [];
    const map = new Map<string, Record<string, string>>();

    for (const img of imageStore.images) {
      const name = img.name.replace(/\.[^.]+$/, '');
      if (prefix && !name.startsWith(prefix)) continue;
      const rest = name.slice(prefix.length);
      for (const s of suffixes) {
        if (!s) continue;
        if (rest.endsWith(s)) {
          const mid = rest.slice(0, rest.length - s.length);
          if (!map.has(mid)) map.set(mid, {});
          map.get(mid)![s] = img.name;
          break;
        }
      }
    }

    const activeSuffixes = suffixes.filter((s) => s);
    return [...map.entries()]
      .map(([middle, slots]) => ({
        middle,
        slots,
        complete: activeSuffixes.every((s) => !!slots[s]),
      }))
      .sort((a, b) => a.middle.localeCompare(b.middle));
  });

  const PREVIEW_LIMIT = 6;
  const previewGroups = $derived(setGroups.slice(0, PREVIEW_LIMIT));
  const hiddenCount = $derived(Math.max(0, setGroups.length - PREVIEW_LIMIT));
  const completeCount = $derived(setGroups.filter((g) => g.complete).length);
</script>

<div class="inspector-set">
  <!-- Prefix -->
  <div class="row">
    <label class="row-label">
      Prefix
      {#if isPrefixWired()}<span class="wired-badge">wired</span>{/if}
    </label>
    {#if isPrefixWired()}
      <div class="wired-value row-input">{getWiredPrefixValue()}</div>
    {:else}
      <input
        class="text-input row-input"
        type="text"
        value={prefix}
        placeholder="e.g. T_"
        oninput={(e) => setPrefix((e.currentTarget as HTMLInputElement).value)}
      />
    {/if}
  </div>

  <div class="section-sep"></div>
  <div class="section-label">Suffixes</div>

  <!-- Suffix list -->
  {#each suffixes as suffix, i}
    {@const wired = isSuffixWired(i)}
    <div class="suffix-row">
      <span class="suffix-label">
        suffix {i + 1}
        {#if wired}<span class="wired-badge">wired</span>{/if}
      </span>
      {#if wired}
        <div class="wired-value">{getWiredSuffixValue(i)}</div>
      {:else}
        <input
          class="text-input"
          type="text"
          value={suffix}
          placeholder="e.g. _AO"
          oninput={(e) => updateSuffix(i, (e.currentTarget as HTMLInputElement).value)}
        />
      {/if}
      <button class="del-btn" onclick={() => removeSuffix(i)} title="Remove suffix">×</button>
    </div>
  {/each}

  <div class="add-row">
    <button class="btn btn--neutral btn--full" onclick={addSuffix}>+ Add Suffix</button>
  </div>

  <!-- Matched sets preview -->
  {#if suffixes.filter((s) => s).length > 0}
    <div class="divider"></div>
    <div class="section-label">
      Matched sets
      {#if setGroups.length > 0}
        <span class="match-count">({completeCount}/{setGroups.length} complete)</span>
      {/if}
    </div>

    {#if setGroups.length === 0}
      <div class="no-match">No images match the current pattern.</div>
    {:else}
      {#each previewGroups as group}
        <div class="set-row" class:complete={group.complete} class:incomplete={!group.complete}>
          <span class="set-middle">{prefix}{group.middle}</span>
          <div class="set-slots">
            {#each suffixes.filter((s) => s) as s}
              <span class="slot" class:found={!!group.slots[s]} class:missing={!group.slots[s]}>
                {s}
              </span>
            {/each}
          </div>
        </div>
      {/each}
      {#if hiddenCount > 0}
        <div class="more-hint">…and {hiddenCount} more</div>
      {/if}
    {/if}
  {/if}
</div>

<style>
  .inspector-set {
    padding: 8px 0;
    display: flex;
    flex-direction: column;
  }

  /* ── Prefix row ── */
  .row {
    display: flex;
    align-items: center;
    padding: 4px 12px;
    gap: 8px;
    min-height: 30px;
  }

  .row-label {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text);
    width: 52px;
    flex-shrink: 0;
  }

  .row-input {
    flex: 1;
  }

  .section-sep {
    height: 1px;
    background: color-mix(in srgb, var(--border) 40%, transparent);
    margin: 4px 0;
  }

  /* ── Section label ── */
  .section-label {
    font-family: var(--font-ui);
    font-size: var(--font-size-xxs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--text-muted);
    padding: 6px 12px 2px;
  }

  .match-count {
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
    color: var(--text-muted);
  }

  /* ── Suffix rows ── */
  .suffix-row {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 25%, transparent);
  }

  .suffix-label {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-bright);
    opacity: 0.6;
    user-select: none;
    flex-shrink: 0;
    width: 52px;
  }

  .wired-badge {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--port-color-string);
    border: 1px solid var(--port-color-string);
    border-radius: 3px;
    padding: 0 3px;
    line-height: 14px;
    margin-left: 6px;
  }

  .suffix-row .text-input {
    flex: 1;
    width: auto;
  }

  .wired-value {
    flex: 1;
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    color: var(--port-color-string);
    opacity: 0.8;
    padding: 3px 0;
  }

  .del-btn {
    height: 30px;
    padding: 0 10px;
    background: color-mix(in srgb, var(--color-danger) 14%, var(--panel-header-bg));
    border: 2px solid color-mix(in srgb, var(--color-danger) 40%, transparent);
    border-radius: 4px;
    color: var(--color-danger-text);
    font-family: var(--font-ui);
    font-size: var(--font-size-xl);
    cursor: pointer;
    outline: none;
    flex-shrink: 0;
    transition:
      border-color 0.1s,
      color 0.1s,
      background 0.1s;
  }

  .del-btn:hover {
    background: color-mix(in srgb, var(--color-danger) 22%, var(--panel-header-bg));
    border-color: color-mix(in srgb, var(--color-danger) 65%, transparent);
    color: var(--color-danger-text-bright);
  }

  /* ── Add row ── */
  .add-row {
    padding: 8px 12px;
  }

  /* ── Divider ── */
  .divider {
    height: 1px;
    background: var(--ctx-separator);
    margin: 6px 0;
  }

  /* ── Set preview ── */
  .no-match {
    padding: 4px 12px 8px;
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-muted);
    font-style: italic;
  }

  .set-row {
    display: flex;
    flex-direction: column;
    padding: 4px 12px;
    gap: 2px;
    border-left: 2px solid transparent;
    margin-bottom: 2px;
  }

  .set-row.complete {
    border-left-color: var(--accent);
  }
  .set-row.incomplete {
    border-left-color: color-mix(in srgb, var(--color-warning) 60%, transparent);
  }

  .set-middle {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--text-bright);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .set-slots {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .slot {
    font-family: var(--font-mono);
    font-size: var(--font-size-xxs);
    padding: 1px 4px;
    border-radius: 2px;
  }

  .slot.found {
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    color: var(--accent);
  }
  .slot.missing {
    background: color-mix(in srgb, var(--color-error) 12%, transparent);
    color: var(--color-error);
  }

  .more-hint {
    padding: 2px 12px 4px;
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-muted);
    font-style: italic;
  }
</style>
