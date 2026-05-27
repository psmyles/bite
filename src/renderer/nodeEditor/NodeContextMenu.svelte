<script lang="ts">
  import type { NodeDefinition } from '../../shared/types.js';

  interface Props {
    x: number;
    y: number;
    /** If set (wire-drop mode), only show nodes whose inputs include this port type */
    filterType: string | null;
    definitions: NodeDefinition[];
    onSelect: (def: NodeDefinition) => void;
    onClose: () => void;
    groupable?: boolean;
    ungroupable?: boolean;
    onGroupSelection?: () => void;
    onUngroup?: () => void;
  }

  let { x, y, filterType, definitions, onSelect, onClose, groupable, ungroupable, onGroupSelection, onUngroup }: Props =
    $props();

  let search = $state('');
  let hoveredCategory = $state<string | null>(null);
  let subMenuY = $state(0); // top of hovered category row
  let subMenuRowBottom = $state(0); // bottom of hovered category row
  let searchEl = $state<HTMLInputElement | undefined>(undefined);
  let listEl = $state<HTMLElement | undefined>(undefined);
  let subListEl = $state<HTMLElement | undefined>(undefined);

  // ── Keyboard navigation state ────────────────────────────────────────────
  let activeIndex = $state(-1); // search mode: index in filtered[]
  let activeCatIndex = $state(-1); // browse mode: index in grouped()
  let activeSubIndex = $state(-1); // browse mode: index in subDefs[]
  let subMenuActive = $state(false); // whether arrow focus is inside the sub-menu

  // Reset navigation whenever search text changes
  $effect(() => {
    search; // reactive dependency
    activeIndex = -1;
  });

  // Scroll active item into view — search list
  $effect(() => {
    if (activeIndex < 0 || !listEl) return;
    const items = listEl.querySelectorAll<HTMLElement>('.ctx-item');
    items[activeIndex]?.scrollIntoView({ block: 'nearest' });
  });

  // Scroll active item into view — sub-menu
  $effect(() => {
    if (activeSubIndex < 0 || !subListEl) return;
    const items = subListEl.querySelectorAll<HTMLElement>('.ctx-item');
    items[activeSubIndex]?.scrollIntoView({ block: 'nearest' });
  });

  // Portal action: moves the element to document.body so that position:fixed
  // is relative to the viewport, not any transformed ancestor (.center-col).
  function portal(el: HTMLElement): { destroy(): void } {
    document.body.appendChild(el);
    return {
      destroy() {
        el.remove();
      },
    };
  }

  // Read menu widths from CSS vars so JS positioning matches the CSS
  function cssVal(name: string, fallback: number): number {
    return parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name)) || fallback;
  }
  const MENU_W = cssVal('--ctx-width', 200);
  const SUB_W = cssVal('--ctx-sub-width', 190);

  // Horizontal clamp
  const menuLeft = $derived(Math.min(x, window.innerWidth - MENU_W - 8));

  // Flip above the cursor when there's more viewport space above than below
  const spaceBelow = $derived(window.innerHeight - y - 8);
  const spaceAbove = $derived(y - 8);
  const isFlipped = $derived(spaceAbove > spaceBelow);
  // Give the panel all available space in the chosen direction — no fixed cap, no scrollbar when room exists
  const actualMaxH = $derived(isFlipped ? spaceAbove : spaceBelow);
  // When flipped, anchor the bottom of the panel to the cursor; otherwise anchor the top
  const panelStyle = $derived(
    isFlipped
      ? `left:${menuLeft}px; bottom:${window.innerHeight - y}px; top:auto; max-height:${actualMaxH}px`
      : `left:${menuLeft}px; top:${y}px; bottom:auto; max-height:${actualMaxH}px`
  );

  // Sub-menu: prefer right side, flip left if near right edge
  const subLeft = $derived(
    menuLeft + MENU_W + 4 + SUB_W <= window.innerWidth ? menuLeft + MENU_W + 4 : menuLeft - SUB_W - 4
  );

  // Sub-menu vertical: anchor to the hovered category row, flip upward if near bottom edge.
  // Use CSS `bottom` when flipping so the browser pins the real rendered bottom to the row edge,
  // rather than estimating height and computing `top` (which drifts when estimate != actual).
  const ITEM_H = 32; // estimate only — used to decide whether to flip, not for positioning
  const subFlipUp = $derived(subMenuY + subDefs.length * ITEM_H + 8 > window.innerHeight);
  const subAvailH = $derived(subFlipUp ? subMenuRowBottom - 8 : window.innerHeight - subMenuY - 8);
  const subMenuStyle = $derived(
    subFlipUp
      ? `left:${subLeft}px; bottom:${window.innerHeight - subMenuRowBottom}px; top:auto; max-height:${subAvailH}px`
      : `left:${subLeft}px; top:${subMenuY}px; bottom:auto; max-height:${subAvailH}px`
  );

  // Auto-focus search on mount
  $effect(() => {
    searchEl?.focus();
  });

  // Filter by port-type compatibility (wire-drop mode).
  const available = $derived(
    filterType
      ? definitions.filter((d) => {
          // 'any' ports are compatible with everything
          if (filterType === 'any') return true;
          // Nodes with 'any' params accept any wire type
          if (d.params.some((p) => p.type === 'any')) return true;
          // Match on inputs (wire from a source handle — drop target needs an input of that type)
          if (d.inputs.some((p) => p.type === filterType)) return true;
          // Match on outputs (wire from a target handle — drop target needs an output of that type)
          if (d.outputs.some((p) => p.type === filterType)) return true;
          // Param-level type aliases
          if (filterType === 'number' || filterType === 'numeric')
            return d.params.some((p) => p.type === 'int' || p.type === 'float' || p.type === 'numeric');
          if (filterType === 'boolean') return d.params.some((p) => p.type === 'bool');
          if (filterType === 'string') return d.params.some((p) => p.type === 'string');
          if (filterType === 'vector2') return d.params.some((p) => p.type === 'vector2');
          if (filterType === 'vector3') return d.params.some((p) => p.type === 'vector3');
          if (filterType === 'vector4') return d.params.some((p) => p.type === 'vector4');
          if (filterType === 'color') return d.params.some((p) => p.type === 'color');
          return false;
        })
      : definitions
  );

  // Filter by search text
  const filtered = $derived.by(() => {
    if (!search.trim()) return available;
    const q = search.toLowerCase();
    return available.filter(
      (d) =>
        d.label.toLowerCase().includes(q) ||
        d.category.toLowerCase().includes(q) ||
        (d.aliases?.some((a) => a.toLowerCase().includes(q)) ?? false)
    );
  });

  // Group into categories (only used when not searching)
  const grouped = $derived(() => {
    const map = new Map<string, NodeDefinition[]>();
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

  // Nodes for the currently hovered/focused category
  const subDefs = $derived(grouped().find((g) => g.category === hoveredCategory)?.defs ?? []);

  // ── Tooltip ───────────────────────────────────────────────────────────────
  let tooltipDef = $state<NodeDefinition | null>(null);
  let tooltipX = $state(0);
  let tooltipY = $state(0);
  let tooltipSide = $state<'left' | 'right'>('right');
  let tooltipTimer: ReturnType<typeof setTimeout> | undefined;

  function onDefEnter(e: MouseEvent, def: NodeDefinition, side: 'left' | 'right' = 'right') {
    clearTimeout(tooltipTimer);
    if (!def.description) return;
    const el = e.currentTarget as HTMLElement;
    tooltipTimer = setTimeout(() => {
      const r = el.getBoundingClientRect();
      tooltipSide = side;
      tooltipX =
        side === 'left'
          ? Math.max(8, r.left - 8) // right edge of tooltip
          : Math.min(r.right + 8, window.innerWidth - 328); // left edge of tooltip
      tooltipY = Math.max(32, Math.min(r.top + r.height / 2, window.innerHeight - 32));
      tooltipDef = def;
    }, 200);
  }

  function onDefLeave() {
    clearTimeout(tooltipTimer);
    tooltipDef = null;
  }

  function onCategoryEnter(cat: string, el: HTMLElement) {
    onDefLeave();
    hoveredCategory = cat;
    const rect = el.getBoundingClientRect();
    subMenuY = rect.top;
    subMenuRowBottom = rect.bottom;
    // Mouse hover resets keyboard sub-menu focus
    subMenuActive = false;
    activeSubIndex = -1;
    // Sync activeCatIndex so arrow keys stay coherent after mousing
    activeCatIndex = grouped().findIndex((g) => g.category === cat);
  }

  function select(def: NodeDefinition) {
    onSelect(def);
    onClose();
  }

  function onSearchKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (subMenuActive) {
        subMenuActive = false;
        activeSubIndex = -1;
      } else {
        e.stopPropagation();
        onClose();
      }
      return;
    }

    if (search.trim()) {
      // ── Search mode navigation ───────────────────────────────────────────
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        activeIndex = Math.min(activeIndex + 1, filtered.length - 1);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        activeIndex = Math.max(activeIndex - 1, 0);
      } else if (e.key === 'Enter') {
        e.preventDefault();
        const idx = activeIndex >= 0 ? activeIndex : filtered.length === 1 ? 0 : -1;
        if (idx >= 0) select(filtered[idx]);
      }
    } else {
      // ── Browse mode navigation ───────────────────────────────────────────
      const cats = grouped();
      if (!subMenuActive) {
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          activeCatIndex = Math.min(activeCatIndex + 1, cats.length - 1);
          const cat = cats[activeCatIndex];
          if (cat) {
            hoveredCategory = cat.category;
            const rows = listEl?.querySelectorAll<HTMLElement>('.ctx-item--cat');
            if (rows?.[activeCatIndex]) {
              const r = rows[activeCatIndex].getBoundingClientRect();
              subMenuY = r.top;
              subMenuRowBottom = r.bottom;
            }
          }
        } else if (e.key === 'ArrowUp') {
          e.preventDefault();
          activeCatIndex = Math.max(activeCatIndex - 1, 0);
          const cat = cats[activeCatIndex];
          if (cat) {
            hoveredCategory = cat.category;
            const rows = listEl?.querySelectorAll<HTMLElement>('.ctx-item--cat');
            if (rows?.[activeCatIndex]) {
              const r = rows[activeCatIndex].getBoundingClientRect();
              subMenuY = r.top;
              subMenuRowBottom = r.bottom;
            }
          }
        } else if (e.key === 'ArrowRight' || e.key === 'Enter') {
          e.preventDefault();
          if (hoveredCategory && subDefs.length > 0) {
            subMenuActive = true;
            activeSubIndex = 0;
          }
        }
      } else {
        // Focus is inside the sub-menu
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          activeSubIndex = Math.min(activeSubIndex + 1, subDefs.length - 1);
        } else if (e.key === 'ArrowUp') {
          e.preventDefault();
          activeSubIndex = Math.max(activeSubIndex - 1, 0);
        } else if (e.key === 'ArrowLeft') {
          e.preventDefault();
          subMenuActive = false;
          activeSubIndex = -1;
        } else if (e.key === 'Enter') {
          e.preventDefault();
          if (activeSubIndex >= 0) select(subDefs[activeSubIndex]);
        }
      }
    }
  }
</script>

<!-- Portal wrapper — teleported to document.body so position:fixed uses the viewport -->
<div use:portal>
  <!-- Backdrop: click outside to close -->
  <div class="ctx-backdrop" onclick={onClose} role="presentation"></div>

  <!-- Main menu panel -->
  <div class="ctx-panel" style={panelStyle} role="menu">
    {#if groupable && onGroupSelection}
      <button class="ctx-action" onclick={onGroupSelection} role="menuitem">
        Group Selection <kbd>Ctrl+G</kbd>
      </button>
      <div class="ctx-action-sep"></div>
    {/if}
    {#if ungroupable && onUngroup}
      <button class="ctx-action" onclick={onUngroup} role="menuitem">
        Ungroup <kbd>Ctrl+Shift+G</kbd>
      </button>
      <div class="ctx-action-sep"></div>
    {/if}

    <input
      class="ctx-search"
      type="text"
      placeholder="Search nodes…"
      autocomplete="off"
      spellcheck="false"
      bind:value={search}
      bind:this={searchEl}
      onkeydown={onSearchKeydown}
    />

    <div
      class="ctx-list"
      bind:this={listEl}
      onscroll={() => {
        hoveredCategory = null;
      }}
    >
      {#if search.trim()}
        <!-- Flat search results with keyboard highlight -->
        {#if filtered.length === 0}
          <div class="ctx-empty">No matching nodes.</div>
        {:else}
          {#each filtered as def, i (def.id)}
            <button
              class="ctx-item"
              class:ctx-item--focused={i === activeIndex}
              onclick={() => select(def)}
              onmouseenter={(e) => onDefEnter(e, def)}
              onmouseleave={onDefLeave}
              role="menuitem"
            >
              <span class="ctx-item-label">{def.label}</span>
              <span class="ctx-item-cat">{def.category}</span>
            </button>
          {/each}
        {/if}
      {:else}
        <!-- Category rows with keyboard highlight -->
        {#each grouped() as group, i (group.category)}
          <button
            class="ctx-item ctx-item--cat"
            class:ctx-item--active={hoveredCategory === group.category}
            class:ctx-item--focused={i === activeCatIndex && !subMenuActive}
            onmouseenter={(e) => onCategoryEnter(group.category, e.currentTarget)}
            role="menuitem"
            aria-haspopup="true"
          >
            <span>{group.category}</span>
            <span class="ctx-arrow">▶</span>
          </button>
        {/each}
      {/if}
    </div>
  </div>

  <!-- Sub-menu flyout -->
  {#if hoveredCategory && !search.trim() && subDefs.length > 0}
    <div class="ctx-panel ctx-panel--sub" style={subMenuStyle} role="menu" bind:this={subListEl}>
      {#each subDefs as def, i (def.id)}
        <button
          class="ctx-item"
          class:ctx-item--focused={i === activeSubIndex && subMenuActive}
          onclick={() => select(def)}
          onmouseenter={(e) => onDefEnter(e, def, subLeft > menuLeft ? 'right' : 'left')}
          onmouseleave={onDefLeave}
          role="menuitem"
        >
          {def.label}
        </button>
      {/each}
    </div>
  {/if}

  {#if tooltipDef}
    <div
      class="node-tooltip-fixed"
      style="{tooltipSide === 'right'
        ? `left:${tooltipX}px`
        : `right:${window.innerWidth - tooltipX}px`}; top:{tooltipY}px; transform:translateY(-50%)"
    >
      {tooltipDef.description}
    </div>
  {/if}
</div>

<!-- end portal wrapper -->

<style>
  .ctx-action {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 7px 10px;
    background: none;
    border: none;
    color: var(--ctx-text);
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    text-align: left;
    cursor: pointer;
    user-select: none;
    gap: 8px;
  }

  .ctx-action:hover {
    background: var(--ctx-item-hover-bg);
    color: var(--text-bright);
  }

  .ctx-action kbd {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ctx-text-muted);
    white-space: nowrap;
  }

  .ctx-action-sep {
    height: 1px;
    background: var(--ctx-separator);
  }

  .ctx-backdrop {
    position: fixed;
    inset: 0;
    z-index: 999;
    background: transparent;
  }

  .ctx-panel {
    position: fixed;
    z-index: 1000;
    width: var(--ctx-width);
    max-height: var(--ctx-max-height);
    display: flex;
    flex-direction: column;
    background: var(--ctx-bg);
    border: 1px solid var(--ctx-border);
    border-radius: var(--ctx-radius);
    box-shadow: var(--ctx-shadow);
    overflow: hidden;
  }

  .ctx-panel--sub {
    width: var(--ctx-sub-width);
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: transparent transparent;
  }

  .ctx-panel--sub:hover {
    scrollbar-color: var(--scrollbar-thumb) transparent;
  }

  .ctx-search {
    flex-shrink: 0;
    width: 100%;
    background: var(--ctx-search-bg);
    border: none;
    border-bottom: 1px solid var(--ctx-separator);
    padding: 8px 10px;
    color: var(--ctx-text);
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    outline: none;
    box-sizing: border-box;
  }

  .ctx-list {
    overflow-y: auto;
    flex: 1;
    scrollbar-width: thin;
    scrollbar-color: transparent transparent;
  }

  .ctx-list:hover {
    scrollbar-color: var(--scrollbar-thumb) transparent;
  }

  .ctx-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    width: 100%;
    padding: 7px 10px;
    background: none;
    border: none;
    color: var(--ctx-text);
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    text-align: left;
    cursor: pointer;
    user-select: none;
    white-space: nowrap;
    overflow: hidden;
  }

  .ctx-item:hover,
  .ctx-item--active,
  .ctx-item--focused {
    background: var(--ctx-item-hover-bg);
  }

  .ctx-item--cat {
    font-weight: 500;
  }

  .ctx-item-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ctx-item-cat {
    font-size: 11px;
    color: var(--ctx-text-muted);
    flex-shrink: 0;
    opacity: 0.85;
  }

  .ctx-arrow {
    font-size: 11px;
    color: var(--ctx-text-muted);
    flex-shrink: 0;
  }

  .ctx-empty {
    padding: 10px;
    color: var(--ctx-text-muted);
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
  }
</style>
