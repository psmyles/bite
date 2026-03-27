<script lang="ts">
  import type { NodeDefinition } from '../../shared/types.js'

  interface Props {
    x: number
    y: number
    /** If set (wire-drop mode), only show nodes whose inputs include this port type */
    filterType: string | null
    definitions: NodeDefinition[]
    onSelect: (def: NodeDefinition) => void
    onClose: () => void
    groupable?: boolean
    ungroupable?: boolean
    onGroupSelection?: () => void
    onUngroup?: () => void
  }

  let { x, y, filterType, definitions, onSelect, onClose, groupable, ungroupable, onGroupSelection, onUngroup }: Props = $props()

  let search         = $state('')
  let hoveredCategory = $state<string | null>(null)
  let subMenuY          = $state(0)   // top of hovered category row
  let subMenuRowBottom  = $state(0)   // bottom of hovered category row
  let searchEl       = $state<HTMLInputElement | undefined>(undefined)
  let listEl         = $state<HTMLElement | undefined>(undefined)
  let subListEl      = $state<HTMLElement | undefined>(undefined)

  // ── Keyboard navigation state ────────────────────────────────────────────
  let activeIndex    = $state(-1)   // search mode: index in filtered[]
  let activeCatIndex = $state(-1)   // browse mode: index in grouped()
  let activeSubIndex = $state(-1)   // browse mode: index in subDefs[]
  let subMenuActive  = $state(false) // whether arrow focus is inside the sub-menu

  // Reset navigation whenever search text changes
  $effect(() => {
    search  // reactive dependency
    activeIndex = -1
  })

  // Scroll active item into view — search list
  $effect(() => {
    if (activeIndex < 0 || !listEl) return
    const items = listEl.querySelectorAll<HTMLElement>('.ctx-item')
    items[activeIndex]?.scrollIntoView({ block: 'nearest' })
  })

  // Scroll active item into view — sub-menu
  $effect(() => {
    if (activeSubIndex < 0 || !subListEl) return
    const items = subListEl.querySelectorAll<HTMLElement>('.ctx-item')
    items[activeSubIndex]?.scrollIntoView({ block: 'nearest' })
  })

  // Portal action: moves the element to document.body so that position:fixed
  // is relative to the viewport, not any transformed ancestor (.center-col).
  function portal(el: HTMLElement): { destroy(): void } {
    document.body.appendChild(el)
    return { destroy() { el.remove() } }
  }

  // Read menu widths from CSS vars so JS positioning matches the CSS
  function cssVal(name: string, fallback: number): number {
    return parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name)) || fallback
  }
  const MENU_W      = cssVal('--ctx-width', 200)
  const SUB_W       = cssVal('--ctx-sub-width', 190)
  const MENU_MAX_H  = cssVal('--ctx-max-height', 360)
  const ACTION_ROW_H = 33  // button (≈32px) + separator (1px)

  // Grow max-height by the space action rows add above the search bar
  const actionCount = $derived((groupable && !!onGroupSelection ? 1 : 0) + (ungroupable && !!onUngroup ? 1 : 0))
  const panelMaxH   = $derived(MENU_MAX_H + actionCount * ACTION_ROW_H)

  // Horizontal clamp
  const menuLeft = $derived(Math.min(x, window.innerWidth - MENU_W - 8))

  // Vertical: flip above cursor if there isn't enough space below
  const menuTop = $derived(
    y + panelMaxH + 8 > window.innerHeight
      ? Math.max(8, y - panelMaxH)
      : y
  )

  // Sub-menu: prefer right side, flip left if near right edge
  const subLeft = $derived(
    menuLeft + MENU_W + 4 + SUB_W <= window.innerWidth
      ? menuLeft + MENU_W + 4
      : menuLeft - SUB_W - 4
  )

  // Sub-menu vertical: anchor to the hovered category row, flip upward if near bottom edge.
  // Use CSS `bottom` when flipping so the browser pins the real rendered bottom to the row edge,
  // rather than estimating height and computing `top` (which drifts when estimate != actual).
  const ITEM_H = 32  // estimate only — used to decide whether to flip, not for positioning
  const subMenuHeight  = $derived(Math.min(subDefs.length * ITEM_H, MENU_MAX_H))
  const subFlipUp      = $derived(subMenuY + subMenuHeight + 8 > window.innerHeight)
  const subMenuStyle   = $derived(
    subFlipUp
      ? `left:${subLeft}px; bottom:${window.innerHeight - subMenuRowBottom}px; top:auto; max-height:${subMenuHeight}px`
      : `left:${subLeft}px; top:${subMenuY}px; bottom:auto; max-height:${subMenuHeight}px`
  )

  // Auto-focus search on mount
  $effect(() => { searchEl?.focus() })

  // Filter by port-type compatibility (wire-drop mode).
  const available = $derived(
    filterType
      ? definitions.filter(d => {
          // 'any' ports are compatible with everything
          if (filterType === 'any') return true
          // Nodes with 'any' params accept any wire type
          if (d.params.some(p => p.type === 'any')) return true
          // Match on inputs (wire from a source handle — drop target needs an input of that type)
          if (d.inputs.some(p => p.type === filterType)) return true
          // Match on outputs (wire from a target handle — drop target needs an output of that type)
          if (d.outputs.some(p => p.type === filterType)) return true
          // Param-level type aliases
          if (filterType === 'number' || filterType === 'numeric')
            return d.params.some(p => p.type === 'int' || p.type === 'float' || p.type === 'numeric')
          if (filterType === 'boolean') return d.params.some(p => p.type === 'bool')
          if (filterType === 'string')  return d.params.some(p => p.type === 'string')
          if (filterType === 'vector2') return d.params.some(p => p.type === 'vector2')
          if (filterType === 'vector3') return d.params.some(p => p.type === 'vector3')
          if (filterType === 'vector4') return d.params.some(p => p.type === 'vector4')
          if (filterType === 'color')   return d.params.some(p => p.type === 'color')
          return false
        })
      : definitions
  )

  // Filter by search text
  const filtered = $derived(
    search.trim()
      ? available.filter(d =>
          d.label.toLowerCase().includes(search.toLowerCase()) ||
          d.category.toLowerCase().includes(search.toLowerCase())
        )
      : available
  )

  // Group into categories (only used when not searching)
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

  // Nodes for the currently hovered/focused category
  const subDefs = $derived(grouped().find(g => g.category === hoveredCategory)?.defs ?? [])

  function onCategoryEnter(cat: string, el: HTMLElement) {
    hoveredCategory = cat
    const rect = el.getBoundingClientRect()
    subMenuY         = rect.top
    subMenuRowBottom = rect.bottom
    // Mouse hover resets keyboard sub-menu focus
    subMenuActive  = false
    activeSubIndex = -1
    // Sync activeCatIndex so arrow keys stay coherent after mousing
    activeCatIndex = grouped().findIndex(g => g.category === cat)
  }

  function select(def: NodeDefinition) {
    onSelect(def)
    onClose()
  }

  function onSearchKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (subMenuActive) { subMenuActive = false; activeSubIndex = -1 }
      else               { e.stopPropagation(); onClose() }
      return
    }

    if (search.trim()) {
      // ── Search mode navigation ───────────────────────────────────────────
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        activeIndex = Math.min(activeIndex + 1, filtered.length - 1)
      } else if (e.key === 'ArrowUp') {
        e.preventDefault()
        activeIndex = Math.max(activeIndex - 1, 0)
      } else if (e.key === 'Enter') {
        e.preventDefault()
        const idx = activeIndex >= 0 ? activeIndex : (filtered.length === 1 ? 0 : -1)
        if (idx >= 0) select(filtered[idx])
      }
    } else {
      // ── Browse mode navigation ───────────────────────────────────────────
      const cats = grouped()
      if (!subMenuActive) {
        if (e.key === 'ArrowDown') {
          e.preventDefault()
          activeCatIndex = Math.min(activeCatIndex + 1, cats.length - 1)
          const cat = cats[activeCatIndex]
          if (cat) {
            hoveredCategory = cat.category
            const rows = listEl?.querySelectorAll<HTMLElement>('.ctx-item--cat')
            if (rows?.[activeCatIndex]) {
              const r = rows[activeCatIndex].getBoundingClientRect()
              subMenuY = r.top; subMenuRowBottom = r.bottom
            }
          }
        } else if (e.key === 'ArrowUp') {
          e.preventDefault()
          activeCatIndex = Math.max(activeCatIndex - 1, 0)
          const cat = cats[activeCatIndex]
          if (cat) {
            hoveredCategory = cat.category
            const rows = listEl?.querySelectorAll<HTMLElement>('.ctx-item--cat')
            if (rows?.[activeCatIndex]) {
              const r = rows[activeCatIndex].getBoundingClientRect()
              subMenuY = r.top; subMenuRowBottom = r.bottom
            }
          }
        } else if (e.key === 'ArrowRight' || e.key === 'Enter') {
          e.preventDefault()
          if (hoveredCategory && subDefs.length > 0) {
            subMenuActive  = true
            activeSubIndex = 0
          }
        }
      } else {
        // Focus is inside the sub-menu
        if (e.key === 'ArrowDown') {
          e.preventDefault()
          activeSubIndex = Math.min(activeSubIndex + 1, subDefs.length - 1)
        } else if (e.key === 'ArrowUp') {
          e.preventDefault()
          activeSubIndex = Math.max(activeSubIndex - 1, 0)
        } else if (e.key === 'ArrowLeft') {
          e.preventDefault()
          subMenuActive  = false
          activeSubIndex = -1
        } else if (e.key === 'Enter') {
          e.preventDefault()
          if (activeSubIndex >= 0) select(subDefs[activeSubIndex])
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
<div class="ctx-panel" style="left: {menuLeft}px; top: {menuTop}px; max-height: {panelMaxH}px" role="menu">

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

  <div class="ctx-list" bind:this={listEl} onscroll={() => { hoveredCategory = null }}>
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
  <div
    class="ctx-panel ctx-panel--sub"
    style={subMenuStyle}
    role="menu"
    bind:this={subListEl}
  >
    {#each subDefs as def, i (def.id)}
      <button
        class="ctx-item"
        class:ctx-item--focused={i === activeSubIndex && subMenuActive}
        onclick={() => select(def)}
        role="menuitem"
      >
        {def.label}
      </button>
    {/each}
  </div>
{/if}

</div><!-- end portal wrapper -->

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
