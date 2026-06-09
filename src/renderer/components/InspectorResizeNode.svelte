<script lang="ts">
  import type { Node } from '@xyflow/svelte';
  import { graphStore } from '../stores/graph.svelte.js';
  import { imageStore } from '../stores/images.svelte.js';
  import Dropdown from './Dropdown.svelte';
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js';

  let { selectedNode }: { selectedNode: Node } = $props();

  const params = $derived(getNodeParams(selectedNode?.data));
  const mode = $derived((params.mode as string) ?? 'absolute');
  const preserve = $derived((params.preserve_aspect as boolean) !== false);
  const anchor = $derived((params.anchor as string) ?? 'width');
  const unit = $derived(mode === 'relative' ? '%' : 'px');

  // Source image dimensions for aspect ratio calculation and resize preview
  const srcImg = $derived(imageStore.selected);
  const srcW = $derived(srcImg?.width ?? 0);
  const srcH = $derived(srcImg?.height ?? 0);
  const srcAR = $derived(srcW > 0 && srcH > 0 ? srcW / srcH : 1);

  // Current width/height values (unified across absolute/relative modes)
  const widthVal = $derived(
    mode === 'relative'
      ? Math.max(1, Number(params.scale_width ?? params.scale ?? 100))
      : Math.max(1, Math.round(Number(params.width ?? 1024)))
  );
  const heightVal = $derived(
    mode === 'relative'
      ? Math.max(1, Number(params.scale_height ?? params.scale ?? 100))
      : Math.max(1, Math.round(Number(params.height ?? 1024)))
  );

  // Resize preview — mirrors ImageMagick's actual resize behaviour:
  //   relative: scale by percent; preserve uses widthVal uniformly
  //   absolute+preserve+anchor=width: resize to width, height scales proportionally
  //   absolute+preserve+anchor=height: resize to height, width scales proportionally
  //   absolute+!preserve: force exact dimensions
  const previewW = $derived.by(() => {
    if (!srcW || !srcH) return null;
    if (mode === 'relative') return Math.round((srcW * widthVal) / 100);
    if (preserve) return anchor === 'height' ? Math.round((srcW * heightVal) / srcH) : widthVal;
    return widthVal;
  });
  const previewH = $derived.by(() => {
    if (!srcW || !srcH) return null;
    if (mode === 'relative') {
      return preserve
        ? Math.round((srcH * widthVal) / 100) // uniform scale
        : Math.round((srcH * heightVal) / 100);
    }
    if (preserve) return anchor === 'height' ? heightVal : Math.round((srcH * widthVal) / srcW);
    return heightVal;
  });

  function set(key: string, val: unknown) {
    graphStore.setParam(selectedNode.id, key, val);
  }

  function onWidthChange(raw: string) {
    const v = mode === 'relative' ? Math.max(1, parseFloat(raw) || 1) : Math.max(1, parseInt(raw) || 1);
    if (mode === 'relative') {
      set('scale_width', v);
      if (preserve) set('scale_height', v);
    } else {
      set('width', v);
      if (preserve && srcW > 0 && srcH > 0) {
        set('height', Math.max(1, Math.round(v / srcAR)));
      }
    }
  }

  function onHeightChange(raw: string) {
    const v = mode === 'relative' ? Math.max(1, parseFloat(raw) || 1) : Math.max(1, parseInt(raw) || 1);
    if (mode === 'relative') {
      set('scale_height', v);
      if (preserve) set('scale_width', v);
    } else {
      set('height', v);
      if (preserve && srcW > 0 && srcH > 0) {
        set('width', Math.max(1, Math.round(v * srcAR)));
      }
    }
  }

  function onPreserveChange(checked: boolean) {
    set('preserve_aspect', checked);
    if (checked) {
      if (mode === 'absolute' && srcW > 0 && srcH > 0) {
        const w = Math.max(1, Math.round(Number(params.width ?? 1024)));
        set('height', Math.max(1, Math.round(w / srcAR)));
      }
      if (mode === 'relative') {
        const sw = Math.max(1, Number(params.scale_width ?? params.scale ?? 100));
        set('scale_height', sw);
      }
    }
    graphStore.updateResizeParamDefs(selectedNode.id, mode, checked, anchor);
  }

  function onAnchorChange(newAnchor: string) {
    set('anchor', newAnchor);
    graphStore.updateResizeParamDefs(selectedNode.id, mode, preserve, newAnchor);
  }
</script>

<!-- Mode -->
<div class="param-row">
  <span class="param-label">Mode</span>
  <Dropdown
    value={mode}
    options={['absolute', 'relative']}
    labels={['Absolute', 'Relative']}
    onchange={(v) => {
      set('mode', v);
      graphStore.updateResizeParamDefs(selectedNode.id, v, preserve, anchor);
    }}
  />
</div>

<!-- Preserve Aspect -->
<div class="param-row param-inline">
  <span class="param-label">Preserve Aspect Ratio</span>
  <input
    type="checkbox"
    checked={preserve}
    onchange={(e) => onPreserveChange((e.target as HTMLInputElement).checked)}
  />
</div>

<!-- Anchor (only when preserve is on and mode is absolute) -->
{#if preserve && mode === 'absolute'}
  <div class="param-row">
    <span class="param-label">Anchor</span>
    <Dropdown value={anchor} options={['width', 'height']} labels={['Width', 'Height']} onchange={onAnchorChange} />
  </div>
{/if}

<!-- Width -->
<div class="param-row">
  <span class="param-label">Width</span>
  <div class="value-row">
    {#if preserve && mode === 'absolute' && anchor === 'height'}
      <input type="number" class="text-input num-input" value={previewW ?? widthVal} disabled />
    {:else}
      <input
        type="number"
        class="text-input num-input"
        value={widthVal}
        min="1"
        step="1"
        onchange={(e) => onWidthChange((e.target as HTMLInputElement).value)}
      />
    {/if}
    <span class="unit">{unit}</span>
  </div>
</div>

<!-- Height -->
<div class="param-row">
  <span class="param-label">Height</span>
  <div class="value-row">
    {#if preserve && mode === 'absolute' && anchor === 'width'}
      <input type="number" class="text-input num-input" value={previewH ?? heightVal} disabled />
    {:else}
      <input
        type="number"
        class="text-input num-input"
        value={heightVal}
        min="1"
        step="1"
        onchange={(e) => onHeightChange((e.target as HTMLInputElement).value)}
      />
    {/if}
    <span class="unit">{unit}</span>
  </div>
</div>

<!-- Resolution -->
<div class="param-row">
  <span class="param-label">Resolution</span>
  <div class="value-row">
    <input
      type="number"
      class="text-input num-input"
      value={Math.max(1, Math.round(Number(params.density ?? 72)))}
      min="1"
      max="9600"
      step="1"
      onchange={(e) => set('density', Math.max(1, parseInt((e.target as HTMLInputElement).value) || 72))}
    />
    <span class="unit">dpi</span>
  </div>
</div>

<!-- Filter -->
<div class="param-row">
  <span class="param-label">Filter</span>
  <Dropdown
    value={(params.filter as string) ?? 'Lanczos'}
    options={['Lanczos', 'Mitchell', 'Catrom', 'Point']}
    onchange={(v) => set('filter', v)}
  />
</div>

<!-- Resize Preview (only when an image is loaded) -->
{#if srcW > 0 && srcH > 0 && previewW !== null && previewH !== null}
  <div class="section">
    <div class="section-title">Resize Preview</div>
    <div class="preview-row">
      <span class="preview-label">Original</span>
      <span class="preview-val">{srcW} × {srcH}</span>
    </div>
    <div class="preview-row">
      <span class="preview-label">Resized</span>
      <span class="preview-val">{previewW} × {previewH}</span>
    </div>
  </div>
{/if}

<style>
  .param-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 7px 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 25%, transparent);
  }

  .param-inline {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    gap: 0;
  }

  .param-label {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-bright);
    opacity: 0.6;
    user-select: none;
  }

  .value-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .num-input {
    flex: 1;
    appearance: textfield;
    -moz-appearance: textfield;
  }

  .num-input::-webkit-outer-spin-button,
  .num-input::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }

  .num-input:disabled {
    opacity: var(--disabled-opacity);
    cursor: default;
  }

  .unit {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--text-bright);
    opacity: 0.4;
    width: 24px;
    flex-shrink: 0;
  }

  /* ── Resize Preview ── */
  .section {
    padding: 10px 12px;
    border-bottom: 1px solid var(--node-border);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .section-title {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-bright);
    opacity: 0.6;
  }

  .preview-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .preview-label {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-bright);
    opacity: 0.5;
  }

  .preview-val {
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    color: var(--text-bright);
  }
</style>
