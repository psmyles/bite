<script lang="ts">
  import type { NodeDefinition } from '../../shared/types.js'

  let { definitions }: { definitions: NodeDefinition[] } = $props()

  let search = $state('')

  // ── Derived: filter + group ────────────────────────────────────────────────
  const filtered = $derived(
    search.trim()
      ? definitions.filter((d) =>
          d.label.toLowerCase().includes(search.toLowerCase()) ||
          d.category.toLowerCase().includes(search.toLowerCase())
        )
      : definitions
  )

  const grouped = $derived(() => {
    const map = new Map<string, NodeDefinition[]>()
    for (const def of filtered) {
      const list = map.get(def.category) ?? []
      list.push(def)
      map.set(def.category, list)
    }
    return [...map.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([cat, defs]) => ({
        category: cat,
        defs: defs.sort((a, b) => a.label.localeCompare(b.label)),
      }))
  })

  // ── Collapsible categories ─────────────────────────────────────────────────
  let collapsed: Set<string> = $state(new Set())

  function toggleCategory(cat: string) {
    const next = new Set(collapsed)
    if (next.has(cat)) next.delete(cat)
    else next.add(cat)
    collapsed = next
  }

  // When searching, always show all matching groups expanded
  function isOpen(cat: string): boolean {
    return !!search.trim() || !collapsed.has(cat)
  }

  // ── Drag ──────────────────────────────────────────────────────────────────
  function onDragStart(e: DragEvent, def: NodeDefinition) {
    if (!e.dataTransfer) return
    e.dataTransfer.effectAllowed = 'copy'
    e.dataTransfer.setData('application/imgplex-node-id', def.id)
    e.dataTransfer.setData('application/imgplex-node-label', def.label)
  }
</script>

<div class="node-library">
  <!-- Header + search -->
  <div class="library-header">Node Library</div>
  <div class="search-wrap">
    <input
      class="search"
      type="search"
      placeholder="Filter nodes…"
      bind:value={search}
      aria-label="Filter nodes"
    />
  </div>

  <!-- Category groups -->
  <div class="categories">
    {#if grouped().length === 0}
      <p class="empty">{search ? 'No matching nodes.' : 'No nodes loaded.'}</p>
    {:else}
      {#each grouped() as group}
        <div class="category">
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="category-label" onclick={() => toggleCategory(group.category)}>
            <span class="collapse-icon">{isOpen(group.category) ? '−' : '+'}</span>
            {group.category}
          </div>
          {#if isOpen(group.category)}
            {#each group.defs as def}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="node-item"
                draggable="true"
                title={def.description ?? def.label}
                ondragstart={(e) => onDragStart(e, def)}
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
    scrollbar-width: thin;
    scrollbar-color: transparent transparent;
    transition: scrollbar-color 0.2s;
  }

  .categories:hover {
    scrollbar-color: var(--scrollbar-thumb) transparent;
  }

  /* Webkit (Chromium/Electron) */
  .categories::-webkit-scrollbar {
    width: var(--scrollbar-width);
  }

  .categories::-webkit-scrollbar-track {
    background: transparent;
  }

  .categories::-webkit-scrollbar-thumb {
    background: transparent;
    border-radius: 3px;
    transition: background 0.2s;
  }

  .categories:hover::-webkit-scrollbar-thumb {
    background: var(--scrollbar-thumb);
  }

  .categories::-webkit-scrollbar-thumb:hover {
    background: var(--scrollbar-thumb-hover);
  }

  .categories::-webkit-scrollbar-button {
    display: none;
  }

  .category {
    margin-top: 4px;
  }

  .category-label {
    display: flex;
    align-items: center;
    gap: 6px;
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
    font-size: 12px;
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
    font-size: 14px;
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
