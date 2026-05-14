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

  // ── Live set preview ──────────────────────────────────────────────────────

  interface SetGroup {
    middle: string;
    slots: Record<string, string | undefined>; // suffix → imageName
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
    <label class="row-label">Prefix</label>
    <input
      class="row-input"
      type="text"
      value={prefix}
      placeholder="e.g. T_"
      oninput={(e) => setPrefix((e.currentTarget as HTMLInputElement).value)}
    />
  </div>

  <div class="section-label">Suffixes</div>

  <!-- Suffix list -->
  {#each suffixes as suffix, i}
    <div class="suffix-row">
      <span class="suffix-index">suffix{i + 1}</span>
      <input
        class="suffix-input"
        type="text"
        value={suffix}
        placeholder="e.g. _AO"
        oninput={(e) => updateSuffix(i, (e.currentTarget as HTMLInputElement).value)}
      />
      <button class="remove-btn" onclick={() => removeSuffix(i)} title="Remove">✕</button>
    </div>
  {/each}

  <div class="add-row">
    <button class="add-btn" onclick={addSuffix}>+ Add suffix</button>
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
    background: var(--input-bg);
    border: 1px solid var(--input-border);
    border-radius: 3px;
    color: var(--text-bright);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 2px 6px;
    height: 22px;
  }

  .row-input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .section-label {
    font-family: var(--font-ui);
    font-size: 10px;
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
    align-items: center;
    gap: 6px;
    padding: 3px 12px;
  }

  .suffix-index {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    width: 32px;
    flex-shrink: 0;
  }

  .suffix-input {
    flex: 1;
    background: var(--input-bg);
    border: 1px solid var(--input-border);
    border-radius: 3px;
    color: var(--text-bright);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 2px 6px;
    height: 22px;
  }

  .suffix-input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .remove-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 11px;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 3px;
    line-height: 1;
    flex-shrink: 0;
  }

  .remove-btn:hover {
    color: var(--text-bright);
    background: var(--ctx-item-hover-bg);
  }

  .add-row {
    padding: 4px 12px;
  }

  .add-btn {
    background: none;
    border: 1px dashed var(--ctx-border);
    color: var(--text-muted);
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    padding: 3px 10px;
    border-radius: 3px;
    cursor: pointer;
    width: 100%;
    transition:
      color 0.12s,
      border-color 0.12s;
  }

  .add-btn:hover {
    color: var(--text-bright);
    border-color: var(--accent);
  }

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
    border-left-color: color-mix(in srgb, #f59e0b 60%, transparent);
  }

  .set-middle {
    font-family: var(--font-mono);
    font-size: 11px;
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
    font-size: 10px;
    padding: 1px 4px;
    border-radius: 2px;
  }

  .slot.found {
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    color: var(--accent);
  }
  .slot.missing {
    background: color-mix(in srgb, #ef4444 12%, transparent);
    color: #ef4444;
  }

  .more-hint {
    padding: 2px 12px 4px;
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-muted);
    font-style: italic;
  }
</style>
