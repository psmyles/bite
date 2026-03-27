<script lang="ts">
  import type { Node } from '@xyflow/svelte'
  import type { NodeDefinition } from '../../shared/types.js'
  import { graphStore } from '../stores/graph.svelte.js'
  import { imageStore } from '../stores/images.svelte.js'
  import { previewRenames, type RenameParams, type NameBlock } from '../../shared/renameUtils.js'

  let {
    definition,
    selectedNode,
  }: { definition: NodeDefinition; selectedNode: Node } = $props()

  const params = $derived(
    ((selectedNode?.data as Record<string, unknown>)?.params ?? {}) as Record<string, unknown>
  )

  const blocks = $derived((params.blocks as NameBlock[] | undefined) ?? [])

  const renameParams = $derived<RenameParams>({ blocks })

  const PREVIEW_LIMIT = 10
  const totalImages   = $derived(imageStore.images.length)
  const imageNames    = $derived(imageStore.images.slice(0, PREVIEW_LIMIT).map((img) => img.name))
  const rows          = $derived(previewRenames(imageNames, renameParams))

  const EXAMPLE_NAMES = ['photo_001.jpg', 'IMG_5432.jpg', 'vacation shot.png']
  const exampleRows   = $derived(previewRenames(EXAMPLE_NAMES, renameParams))

  function setBlocks(b: NameBlock[]) {
    graphStore.setParam(selectedNode.id, 'blocks', b)
  }

  function addText()    { setBlocks([...blocks, { type: 'text',    value: '' }]) }
  function addNumber()  { setBlocks([...blocks, { type: 'number',  start: 1, pad: 2 }]) }
  function addOldName() { setBlocks([...blocks, { type: 'oldname', find: '', replace_with: '' }]) }

  function deleteBlock(i: number) {
    setBlocks(blocks.filter((_, idx) => idx !== i))
  }

  function updateBlock(i: number, patch: Partial<NameBlock>) {
    setBlocks(blocks.map((b, idx) => idx === i ? { ...b, ...patch } as NameBlock : b))
  }

  function onStr(i: number, field: string, e: Event) {
    updateBlock(i, { [field]: (e.target as HTMLInputElement).value } as Partial<NameBlock>)
  }

  function onInt(i: number, field: 'start' | 'pad', e: Event) {
    const n   = parseInt((e.target as HTMLInputElement).value)
    const min = field === 'pad' ? 1 : 0
    updateBlock(i, { [field]: isNaN(n) ? min : Math.max(min, n) } as Partial<NameBlock>)
  }

  // ── Drag-to-reorder ────────────────────────────────────────────────────────
  let dragIdx     = $state<number | null>(null)
  let dragOverPos = $state<number | null>(null)

  // Live-reordered display: remove dragged block from original slot, insert at dragOverPos
  const displayBlocks = $derived.by(() => {
    const indexed = blocks.map((block, i) => ({ block, origIdx: i }))
    if (dragIdx === null || dragOverPos === null || dragIdx === dragOverPos) return indexed
    const [moved] = indexed.splice(dragIdx, 1)
    indexed.splice(dragOverPos, 0, moved)
    return indexed
  })

  function onDragStart(i: number, e: DragEvent) {
    dragIdx = i
    if (e.dataTransfer) { e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', String(i)) }
  }

  // Dragover on the container: use Y geometry to find target slot — no per-item handlers
  function onListDragOver(e: DragEvent) {
    e.preventDefault()
    if (dragIdx === null) return
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
    const container = e.currentTarget as HTMLElement
    const items = Array.from(container.children) as HTMLElement[]
    let pos = items.length
    for (let i = 0; i < items.length; i++) {
      const rect = items[i].getBoundingClientRect()
      if (e.clientY < rect.top + rect.height / 2) { pos = i; break }
    }
    dragOverPos = pos
  }

  function onListDrop(e: DragEvent) {
    e.preventDefault()
    if (dragIdx !== null && dragOverPos !== null && dragIdx !== dragOverPos) {
      const next = [...blocks]
      const [moved] = next.splice(dragIdx, 1)
      next.splice(dragOverPos, 0, moved)
      setBlocks(next)
    }
    dragIdx = dragOverPos = null
  }

  function onDragEnd() { dragIdx = dragOverPos = null }
</script>

<div class="rename-inspector">

  <!-- ── Block builder ─────────────────────────────────────── -->
  <div class="section">
    <div class="section-title">Name Blocks</div>

    {#if blocks.length === 0}
      <div class="empty-blocks">Add blocks below to build a new filename</div>
    {:else}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="block-list" ondragover={onListDragOver} ondrop={onListDrop}>
        {#each displayBlocks as { block, origIdx } (origIdx)}
          {@const isDragging = dragIdx === origIdx}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="block-row"
            class:dragging={isDragging}
            draggable={true}
            ondragstart={(e) => onDragStart(origIdx, e)}
            ondragend={onDragEnd}
          >
            <span class="drag-handle" title="Drag to reorder">
              <svg width="6" height="10" viewBox="0 0 6 10" fill="currentColor">
                <circle cx="1.5" cy="1.5" r="1.2"/><circle cx="4.5" cy="1.5" r="1.2"/>
                <circle cx="1.5" cy="5"   r="1.2"/><circle cx="4.5" cy="5"   r="1.2"/>
                <circle cx="1.5" cy="8.5" r="1.2"/><circle cx="4.5" cy="8.5" r="1.2"/>
              </svg>
            </span>

            {#if block.type === 'text'}
              <span class="block-badge block-badge--text">TEXT</span>
              <input
                class="field-input block-input"
                type="text"
                value={block.value}
                oninput={(e) => onStr(origIdx, 'value', e)}
                placeholder="text…"
                spellcheck="false"
              />

            {:else if block.type === 'number'}
              <span class="block-badge block-badge--num">NUM</span>
              <div class="num-fields">
                <span class="sub-label">start</span>
                <input class="field-input field-input--tiny" type="number" min="0" value={block.start} oninput={(e) => onInt(origIdx, 'start', e)} />
                <span class="sub-label">pad</span>
                <input class="field-input field-input--tiny" type="number" min="1" max="8" value={block.pad} oninput={(e) => onInt(origIdx, 'pad', e)} />
                <span class="num-preview">{String(block.start).padStart(block.pad, '0')}</span>
              </div>

            {:else}
              <!-- oldname block -->
              <span class="block-badge block-badge--orig">ORIG</span>
              <div class="orig-fields">
                <input
                  class="field-input orig-input"
                  type="text"
                  value={block.find}
                  oninput={(e) => onStr(origIdx, 'find', e)}
                  placeholder="find…"
                  spellcheck="false"
                />
                {#if block.find}
                  <span class="orig-arrow">→</span>
                  <input
                    class="field-input orig-input"
                    type="text"
                    value={block.replace_with}
                    oninput={(e) => onStr(origIdx, 'replace_with', e)}
                    placeholder="replace…"
                    spellcheck="false"
                  />
                {/if}
              </div>
            {/if}

            <button class="block-delete" onclick={() => deleteBlock(origIdx)} title="Remove block">×</button>
          </div>
        {/each}
      </div>
    {/if}

    <div class="add-bar">
      <button class="add-btn" onclick={addText}>+ Text</button>
      <button class="add-btn add-btn--num" onclick={addNumber}>+ Number</button>
      <button class="add-btn add-btn--orig" onclick={addOldName}>+ Old Name</button>
    </div>
  </div>

  <!-- ── Preview table ─────────────────────────────────────── -->
  <div class="section preview-section">
    <div class="section-title">
      {totalImages > 0
        ? totalImages > PREVIEW_LIMIT
          ? `Preview — first ${PREVIEW_LIMIT} of ${totalImages} files`
          : `Preview — ${totalImages} file${totalImages > 1 ? 's' : ''}`
        : 'Preview (no files loaded)'}
    </div>
    <div class="preview-table">
      <div class="preview-head">
        <span>Original</span>
        <span>New Name</span>
      </div>
      {#each (imageNames.length > 0 ? rows : exampleRows) as row}
        <div class="preview-row">
          <span class="preview-name preview-name--old">{row.original}</span>
          <span class="preview-arrow">→</span>
          <span class="preview-name preview-name--new" class:unchanged={!row.changed}>{row.newName}</span>
        </div>
      {/each}
      {#if imageNames.length === 0}
        <div class="preview-example-note">Example filenames shown above</div>
      {/if}
    </div>
  </div>

</div>

<style>
  .rename-inspector {
    display: flex;
    flex-direction: column;
  }

  /* ── Sections ── */
  .section {
    padding: 10px 12px 10px;
    border-bottom: 1px solid var(--node-border, rgba(255,255,255,0.07));
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .section-title {
    font-family: var(--font-mono);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-bright);
    opacity: 0.5;
    margin-bottom: 2px;
  }

  /* ── Inputs ── */
  .field-input {
    background: var(--input-bg, rgba(255,255,255,0.06));
    border: 1px solid var(--input-border, rgba(255,255,255,0.12));
    border-radius: 3px;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 4px 7px;
    outline: none;
    min-width: 0;
    transition: border-color 0.12s;
  }
  .field-input:focus { border-color: var(--accent); }
  .field-input--tiny { flex: 0 0 46px; padding: 4px 5px; }

  /* ── Block list ── */
  .empty-blocks {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.35;
    text-align: center;
    padding: 6px 0 2px;
    font-style: italic;
  }

  .block-list { display: flex; flex-direction: column; gap: 3px; }

  .block-row {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 3px 4px 2px;
    border-radius: 3px;
    border: 1px solid transparent;
    transition: background 0.1s, border-color 0.1s;
  }
  .block-row:hover { background: rgba(255,255,255,0.04); }
  .block-row.dragging { opacity: 0.5; border-color: var(--accent); }

  .drag-handle {
    color: var(--text-bright);
    opacity: 0.3;
    cursor: grab;
    flex-shrink: 0;
    padding: 0 2px;
    line-height: 0;
  }
  .drag-handle:hover { opacity: 0.7; }

  /* ── Badges ── */
  .block-badge {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    padding: 2px 5px;
    border-radius: 2px;
    flex-shrink: 0;
  }
  .block-badge--text { background: color-mix(in srgb, var(--accent) 18%, transparent); color: var(--accent); }
  .block-badge--num  { background: color-mix(in srgb, #f59e0b 18%, transparent); color: #f59e0b; }
  .block-badge--orig { background: color-mix(in srgb, #b89cfb 18%, transparent); color: #b89cfb; }

  /* ── Text block ── */
  .block-input { flex: 1; }

  /* ── Number block ── */
  .num-fields {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }
  .sub-label {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.5;
    flex-shrink: 0;
  }
  .num-preview {
    font-family: var(--font-mono);
    font-size: 11px;
    color: #f59e0b;
    opacity: 0.85;
    margin-left: 3px;
    flex-shrink: 0;
  }

  /* ── Old Name block ── */
  .orig-fields {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }
  .orig-input { flex: 1; min-width: 0; }
  .orig-arrow {
    font-size: 11px;
    color: #b89cfb;
    opacity: 0.6;
    flex-shrink: 0;
  }

  /* ── Delete button ── */
  .block-delete {
    background: none;
    border: none;
    color: var(--text-bright);
    opacity: 0.3;
    cursor: pointer;
    font-size: 15px;
    line-height: 1;
    padding: 0 2px;
    flex-shrink: 0;
    transition: opacity 0.12s, color 0.12s;
  }
  .block-delete:hover { opacity: 1; color: #f87171; }

  /* ── Add bar ── */
  .add-bar { display: flex; gap: 5px; }

  .add-btn {
    flex: 1;
    background: rgba(255,255,255,0.05);
    border: 1px solid var(--input-border, rgba(255,255,255,0.12));
    border-radius: 3px;
    color: var(--text-bright);
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 5px 0;
    cursor: pointer;
    transition: background 0.12s, border-color 0.12s, color 0.12s;
  }
  .add-btn:hover {
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    border-color: var(--accent);
    color: var(--text);
  }
  .add-btn--num:hover  { background: color-mix(in srgb, #f59e0b 15%, transparent); border-color: #f59e0b; }
  .add-btn--orig:hover { background: color-mix(in srgb, #b89cfb 15%, transparent); border-color: #b89cfb; }

  /* ── Preview ── */
  .preview-section { gap: 4px; }

  .preview-table {
    display: flex;
    flex-direction: column;
    gap: 1px;
    max-height: 240px;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: transparent transparent;
    transition: scrollbar-color 0.2s;
  }
  .preview-table:hover { scrollbar-color: var(--scrollbar-thumb) transparent; }

  .preview-head {
    display: flex;
    justify-content: space-between;
    padding: 0 2px 3px;
    font-family: var(--font-mono);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-bright);
    opacity: 0.4;
    border-bottom: 1px solid var(--node-border, rgba(255,255,255,0.07));
  }

  .preview-row {
    display: grid;
    grid-template-columns: 1fr 12px 1fr;
    align-items: center;
    gap: 4px;
    padding: 3px 2px;
  }

  .preview-arrow { font-size: 11px; color: var(--text-bright); opacity: 0.3; text-align: center; }

  .preview-name {
    font-family: var(--font-mono);
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .preview-name--old { color: var(--text-bright); opacity: 0.5; }
  .preview-name--new { color: var(--accent); }
  .preview-name--new.unchanged { color: var(--text-bright); opacity: 0.35; }

  .preview-example-note {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.3;
    text-align: center;
    padding: 4px 0 2px;
    font-style: italic;
  }
</style>
