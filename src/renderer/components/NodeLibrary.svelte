<script lang="ts">
  import { SvelteMap, SvelteSet } from 'svelte/reactivity';
  import type { NodeDefinition } from '../../shared/types.js';

  let { definitions }: { definitions: NodeDefinition[] } = $props();

  let search = $state('');

  // ── Tooltip ───────────────────────────────────────────────────────────────
  let tooltipDef = $state<NodeDefinition | null>(null);
  let tooltipX = $state(0);
  let tooltipY = $state(0);
  let tooltipTimer: ReturnType<typeof setTimeout> | undefined;

  function portal(el: HTMLElement): { destroy(): void } {
    document.body.appendChild(el);
    return {
      destroy() {
        el.remove();
      },
    };
  }

  function onItemEnter(e: MouseEvent, def: NodeDefinition) {
    clearTimeout(tooltipTimer);
    if (!def.description) return;
    const el = e.currentTarget as HTMLElement;
    tooltipTimer = setTimeout(() => {
      const r = el.getBoundingClientRect();
      tooltipX = Math.min(r.right + 8, window.innerWidth - 328);
      tooltipY = Math.max(32, Math.min(r.top + r.height / 2, window.innerHeight - 32));
      tooltipDef = def;
    }, 200);
  }

  function onItemLeave() {
    clearTimeout(tooltipTimer);
    tooltipDef = null;
  }

  // ── Derived: filter + group ────────────────────────────────────────────────
  const filtered = $derived.by(() => {
    if (!search.trim()) return definitions;
    const q = search.toLowerCase();
    return definitions.filter(
      (d) =>
        d.label.toLowerCase().includes(q) ||
        d.category.toLowerCase().includes(q) ||
        (d.aliases?.some((a) => a.toLowerCase().includes(q)) ?? false)
    );
  });

  const grouped = $derived(() => {
    const map = new SvelteMap<string, NodeDefinition[]>();
    for (const def of filtered) {
      const list = map.get(def.category) ?? [];
      list.push(def);
      map.set(def.category, list);
    }
    return [...map.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([cat, defs]) => ({
        category: cat,
        defs: defs.sort((a, b) => a.label.localeCompare(b.label)),
      }));
  });

  // ── Collapsible categories ─────────────────────────────────────────────────
  const collapsed = new SvelteSet<string>();

  function toggleCategory(cat: string) {
    if (collapsed.has(cat)) collapsed.delete(cat);
    else collapsed.add(cat);
  }

  // When searching, always show all matching groups expanded
  function isOpen(cat: string): boolean {
    return !!search.trim() || !collapsed.has(cat);
  }

  // ── Drag (JSON-defined nodes) ──────────────────────────────────────────────
  function onDragStart(e: DragEvent, def: NodeDefinition) {
    if (!e.dataTransfer) return;
    e.dataTransfer.effectAllowed = 'copy';
    e.dataTransfer.setData('application/imgplex-node-id', def.id);
    e.dataTransfer.setData('application/imgplex-node-label', def.label);
  }

  // ── Workflow nodes (always visible, not filtered by search) ───────────────
  const WORKFLOW_NODES = [
    { type: 'inputNode', label: 'Input', desc: 'Source of images for the workflow' },
    { type: 'imageOutputNode', label: 'Image Output', desc: 'Write processed images to disk' },
    { type: 'textOutputNode', label: 'Text Output', desc: 'Write text/metadata values to a file' },
    { type: 'flipbookOutputNode', label: 'Flipbook Output', desc: 'Assemble images into a flipbook atlas' },
  ] as const;

  function onWorkflowDragStart(e: DragEvent, nodeType: string) {
    if (!e.dataTransfer) return;
    e.dataTransfer.effectAllowed = 'copy';
    e.dataTransfer.setData('application/imgplex-node-type', nodeType);
  }

  let workflowCollapsed = $state(false);
</script>

<div class="node-library">
  <!-- Header + search -->
  <div class="library-header">Node Library</div>
  <div class="search-wrap">
    <input class="search" type="search" placeholder="Filter nodes…" bind:value={search} aria-label="Filter nodes" />
  </div>

  <!-- Category groups -->
  <div class="categories scrollable">
    <!-- ── Workflow section (pinned at top, not filtered by search) ── -->
    {#if !search.trim()}
      <div class="category">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="category-label" onclick={() => (workflowCollapsed = !workflowCollapsed)}>
          <span class="collapse-icon">{workflowCollapsed ? '+' : '−'}</span>
          Workflow
        </div>
        {#if !workflowCollapsed}
          {#each WORKFLOW_NODES as wn (wn.type)}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="node-item" draggable="true" ondragstart={(e) => onWorkflowDragStart(e, wn.type)}>
              <span class="node-label">{wn.label}</span>
              <span class="drag-hint">⠿</span>
            </div>
          {/each}
        {/if}
      </div>
    {/if}

    {#if grouped().length === 0}
      <p class="empty">{search ? 'No matching nodes.' : 'No nodes loaded.'}</p>
    {:else}
      {#each grouped() as group (group.category)}
        <div class="category">
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="category-label" onclick={() => toggleCategory(group.category)}>
            <span class="collapse-icon">{isOpen(group.category) ? '−' : '+'}</span>
            {group.category}
          </div>
          {#if isOpen(group.category)}
            {#each group.defs as def (def.id)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="node-item"
                draggable="true"
                onmouseenter={(e) => onItemEnter(e, def)}
                onmouseleave={onItemLeave}
                ondragstart={(e) => {
                  onItemLeave();
                  onDragStart(e, def);
                }}
              >
                <span class="node-label">{def.label}</span>
                <span class="drag-hint">⠿</span>
              </div>
            {/each}
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>

{#if tooltipDef}
  <div
    use:portal
    class="node-tooltip-fixed lib-tooltip"
    style="left:{tooltipX}px; top:{tooltipY}px; transform:translateY(-50%)"
  >
    {tooltipDef.description}
  </div>
{/if}

<style>
  .node-library {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--panel-bg);
    overflow: hidden;
  }

  .library-header {
    padding: 10px 12px 10px;
    font-family: var(--text-panel-header-family);
    font-size: var(--text-panel-header-size);
    font-weight: var(--text-panel-header-weight);
    text-transform: var(--text-panel-header-transform);
    letter-spacing: var(--text-panel-header-spacing);
    color: var(--text-bright);
    background: var(--panel-header-bg);
    flex-shrink: 0;
  }

  .search-wrap {
    padding: 8px 10px;
    background: var(--panel-header-bg);
    flex-shrink: 0;
  }

  .search {
    width: 100%;
    background: var(--search-bg);
    border: none;
    border-radius: var(--library-search-radius);
    padding: 6px 10px;
    font-family: var(--text-search-family);
    font-size: var(--text-search-size);
    font-weight: var(--text-search-weight);
    text-transform: var(--text-search-transform);
    letter-spacing: var(--text-search-spacing);
    color: var(--text);
    outline: none;
  }

  .search:focus {
    box-shadow: 0 0 0 1px var(--accent);
  }

  .categories {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0 8px;
  }

  .category {
    margin-top: 4px;
  }

  .category-label {
    display: flex;
    align-items: center;
    gap: var(--panel-gap);
    padding: 4px 12px 3px;
    font-family: var(--text-category-label-family);
    font-size: var(--text-category-label-size);
    font-weight: var(--text-category-label-weight);
    text-transform: var(--text-category-label-transform);
    letter-spacing: var(--text-category-label-spacing);
    color: var(--text-bright);
    cursor: pointer;
    user-select: none;
  }

  .category-label:hover {
    color: var(--text);
  }

  .collapse-icon {
    font-size: var(--font-size-sm);
    line-height: 1;
    width: 10px;
    text-align: center;
    flex-shrink: 0;
    font-family: var(--font-mono);
  }

  .node-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 3px 12px;
    font-family: var(--text-node-item-family);
    font-size: var(--text-node-item-size);
    font-weight: var(--text-node-item-weight);
    text-transform: var(--text-node-item-transform);
    letter-spacing: var(--text-node-item-spacing);
    color: var(--text);
    cursor: grab;
    border-radius: 3px;
    margin: 0 6px;
    transition: background 0.1s;
  }

  .node-item:hover {
    background: var(--library-item-hover-bg);
  }

  .node-item:active {
    cursor: grabbing;
  }

  .drag-hint {
    font-size: var(--font-size-md);
    color: var(--text-bright);
    opacity: 0.5;
    flex-shrink: 0;
  }

  .empty {
    padding: 16px 12px;
    font-family: var(--text-hint-family);
    font-size: var(--text-hint-size);
    font-weight: var(--text-hint-weight);
    color: var(--text-bright);
    margin: 0;
  }
</style>
