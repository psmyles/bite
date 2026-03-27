<script lang="ts">
  import { imageStore } from '../stores/images.svelte.js'

  let stripEl       = $state<HTMLElement | undefined>(undefined)
  let stripHeight   = $state(0)
  let stripWidth    = $state(0)
  let scrollLeft    = $state(0)
  let isDragOver    = $state(false)

  // Measure strip height (for thumbSize) and width (for visible range calculation).
  $effect(() => {
    if (!stripEl) return
    const ro = new ResizeObserver((entries) => {
      stripHeight = entries[0].contentRect.height
      stripWidth  = entries[0].contentRect.width
    })
    ro.observe(stripEl)
    return () => ro.disconnect()
  })

  // Strip padding (top 5 + bottom 4) + thumb padding (3*2) + gap (3) + name (~13px)
  const CHROME    = 28
  const OVERSCAN  = 8    // items to render outside the visible window on each side
  const STRIP_PAD = 8    // .thumb-strip left/right padding

  const thumbSize = $derived(Math.max(32, stripHeight - CHROME))
  // Per-item stride in the flex row: image + padding (3*2) + border (1*2) + gap (4)
  const itemWidth = $derived(thumbSize + 12)

  // Virtual window: indices of items to actually render
  const visibleRange = $derived.by(() => {
    const total = imageStore.images.length
    if (total === 0) return { start: 0, end: -1 }
    const first = Math.floor(Math.max(0, scrollLeft - STRIP_PAD) / itemWidth)
    const last  = Math.ceil((scrollLeft + stripWidth - STRIP_PAD) / itemWidth)
    return {
      start: Math.max(0, first - OVERSCAN),
      end:   Math.min(total - 1, last + OVERSCAN),
    }
  })

  // Spacers maintain the total scrollable width without rendering off-screen items
  const spacerLeft  = $derived(visibleRange.start * itemWidth)
  const spacerRight = $derived(
    Math.max(0, (imageStore.images.length - 1 - visibleRange.end) * itemWidth)
  )

  // Non-passive wheel listener — redirects vertical scroll to horizontal.
  $effect(() => {
    if (!stripEl) return
    function onWheel(e: WheelEvent) {
      if (Math.abs(e.deltaX) > Math.abs(e.deltaY)) return
      e.preventDefault()
      stripEl!.scrollLeft += e.deltaY
    }
    stripEl.addEventListener('wheel', onWheel, { passive: false })
    return () => stripEl!.removeEventListener('wheel', onWheel)
  })

  function getImagePaths(dt: DataTransfer): string[] {
    if (!dt.types.includes('Files')) return []
    return [...dt.files]
      .filter((f) => /\.(jpe?g|png|gif|webp|avif|svg|svgz|ico|bmp|tiff?|heic|heif|jp2|j2k|jpf|jpx|jxl|psd|psb|exr|hdr|dpx|cin|cr[23]|nef|nrw|arw|dng|orf|raf|rw2|pef|srw|x3f|3fr|kdc|mrw|erf|rwl|tga|pcx|pp[mbgn]|pnm|sgi|rgb[a]?|miff|mng|jng|xbm|xpm|xwd|sun|iff|lbm|wbmp|pict?|dds|fits|fts)$/i.test(f.name))
      .map((f) => (f as unknown as { path: string }).path)
      .filter(Boolean)
  }

  function onDragOver(e: DragEvent) {
    if (!e.dataTransfer?.types.includes('Files')) return
    e.preventDefault()
    e.dataTransfer.dropEffect = 'copy'
    isDragOver = true
  }

  function onDragLeave(e: DragEvent) {
    if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) {
      isDragOver = false
    }
  }

  function onDrop(e: DragEvent) {
    e.preventDefault()
    isDragOver = false
    if (!e.dataTransfer) return
    const paths = getImagePaths(e.dataTransfer)
    if (paths.length) imageStore.add(paths)
  }

  const countLabel = $derived(
    imageStore.images.length === 1 ? '1 image' : `${imageStore.images.length} images`
  )
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="filmstrip-panel"
  class:drag-over={isDragOver}
  ondragover={onDragOver}
  ondragleave={onDragLeave}
  ondrop={onDrop}
>
  <!-- ── Thumbnail strip ── -->
  <div
    class="thumb-strip"
    bind:this={stripEl}
    onscroll={(e) => { scrollLeft = (e.currentTarget as HTMLElement).scrollLeft }}
  >
    {#if imageStore.images.length === 0}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="empty-hint" onclick={() => imageStore.openDialog()}>
        Drop images here or click to open…
      </span>
    {:else}
      <!-- Left spacer fills the width of all off-screen items to the left -->
      {#if spacerLeft > 0}
        <div class="virt-spacer" style="width:{spacerLeft}px"></div>
      {/if}

      {#each imageStore.images.slice(visibleRange.start, visibleRange.end + 1) as img, relIdx (img.path)}
        {@const i = visibleRange.start + relIdx}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="thumb"
          class:selected={i === imageStore.selectedIndex}
          onclick={() => imageStore.select(i)}
          title={img.name}
        >
          {#if img.thumbnailDataUrl}
            <img
              src={img.thumbnailDataUrl}
              alt={img.name}
              style="width:{thumbSize}px; height:{thumbSize}px"
            />
          {:else}
            <div class="thumb-placeholder" style="width:{thumbSize}px; height:{thumbSize}px"></div>
          {/if}
          <span class="name" style="max-width:{thumbSize}px">{img.name}</span>
        </div>
      {/each}

      <!-- Right spacer fills the width of all off-screen items to the right -->
      {#if spacerRight > 0}
        <div class="virt-spacer" style="width:{spacerRight}px"></div>
      {/if}
    {/if}
  </div>

  <!-- ── Status bar ── -->
  <div class="status-bar">
    <span class="count">{countLabel}</span>
  </div>
</div>

<style>
  .filmstrip-panel {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--panel-bg);
    transition: background 0.15s;
  }

  .filmstrip-panel.drag-over {
    background: color-mix(in srgb, var(--accent) 12%, var(--panel-bg));
    outline: 1px dashed var(--accent);
    outline-offset: -3px;
  }

  /* ── Thumb strip ── */
  .thumb-strip {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 4px;
    padding: 5px 8px 4px;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: thin;
    scrollbar-color: transparent transparent;
    transition: scrollbar-color 0.2s;
  }

  .thumb-strip:hover { scrollbar-color: var(--scrollbar-thumb) transparent; }
  .thumb-strip::-webkit-scrollbar { height: var(--scrollbar-width); }
  .thumb-strip::-webkit-scrollbar-track { background: transparent; }
  .thumb-strip::-webkit-scrollbar-thumb { background: transparent; border-radius: 3px; }
  .thumb-strip:hover::-webkit-scrollbar-thumb { background: var(--scrollbar-thumb); }
  .thumb-strip::-webkit-scrollbar-thumb:hover { background: var(--scrollbar-thumb-hover); }
  .thumb-strip::-webkit-scrollbar-button { display: none; }

  .empty-hint {
    font-family: var(--text-hint-family);
    font-size: var(--text-hint-size);
    font-weight: var(--text-hint-weight);
    color: var(--text-bright);
    white-space: nowrap;
    margin: auto;
    cursor: pointer;
    opacity: 0.5;
    transition: opacity 0.15s;
  }

  .empty-hint:hover { opacity: 1; }

  /* ── Virtual scroll spacers ── */
  .virt-spacer {
    flex-shrink: 0;
    align-self: stretch;
  }

  /* ── Thumbnails — size set via inline style from thumbSize ── */
  .thumb {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    flex-shrink: 0;
    padding: 3px;
    border-radius: 4px;
    border: 1px solid transparent;
    transition: border-color 0.1s;
    cursor: pointer;
  }

  .thumb:hover { border-color: var(--border); }
  .thumb.selected { border-color: var(--accent); }

  .thumb img,
  .thumb-placeholder {
    object-fit: cover;
    border-radius: 2px;
    display: block;
    flex-shrink: 0;
  }

  .thumb-placeholder { background: var(--panel-header-bg); }

  .name {
    flex-shrink: 0;
    font-family: var(--text-thumb-name-family);
    font-size: 11px;
    font-weight: var(--text-thumb-name-weight);
    color: var(--text-bright);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0.6;
  }

  .thumb.selected .name { opacity: 1; }

  /* ── Status bar ── */
  .status-bar {
    flex-shrink: 0;
    height: 24px;
    display: flex;
    align-items: center;
    padding: 0 10px;
    background: var(--panel-header-bg);
    border-top: 1px solid color-mix(in srgb, var(--border) 40%, transparent);
  }

  .count {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.5;
  }
</style>
