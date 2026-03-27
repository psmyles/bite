<script lang="ts">
  import { untrack } from 'svelte'
  import { NodeResizer } from '@xyflow/svelte'
  import { graphStore } from '../stores/graph.svelte.js'

  interface NodeData {
    params?: Record<string, unknown>
  }

  let {
    id = '',
    data,
    selected = false,
    width,
    height,
  }: { id?: string; data: NodeData; selected?: boolean; width?: number; height?: number } = $props()

  let localName = $state(untrack(() => (data.params?.name ?? 'Group') as string))
  let editing   = $state(false)
  let inputEl   = $state<HTMLInputElement | undefined>(undefined)

  // Sync from store on external changes (undo/redo, load)
  $effect(() => {
    const n = (data.params?.name ?? 'Group') as string
    untrack(() => { if (n !== localName) localName = n })
  })

  // Focus input after Svelte renders the edit branch
  $effect(() => {
    if (!editing) return
    Promise.resolve().then(() => inputEl?.focus())
  })

  // Exit edit on outside click
  $effect(() => {
    if (!editing) return
    function onDocMousedown(e: MouseEvent) {
      if (!(e.target as Element).closest('.group-label-wrap')) editing = false
    }
    document.addEventListener('mousedown', onDocMousedown, true)
    return () => document.removeEventListener('mousedown', onDocMousedown, true)
  })

  function onKeydown(e: KeyboardEvent) {
    e.stopPropagation()
    if (e.key === 'Escape' || e.key === 'Enter') editing = false
  }

  function enterEditing(e: MouseEvent) {
    e.stopPropagation()
    editing = true
  }
</script>

<NodeResizer minWidth={120} minHeight={80} isVisible={selected} />

<!-- Header — floats above the group box (position:absolute, top:-36px).
     Height 36px is hardcoded here; keep in sync if changed. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="group-label-wrap" class:selected>
  {#if editing}
    <input
      class="nodrag group-label-input"
      type="text"
      bind:value={localName}
      bind:this={inputEl}
      oninput={() => graphStore.setParam(id, 'name', localName)}
      onkeydown={onKeydown}
      onclick={(e) => e.stopPropagation()}
      ondblclick={(e) => e.stopPropagation()}
      spellcheck="false"
    />
  {:else}
    <div class="group-label" ondblclick={enterEditing}>{localName}</div>
  {/if}
</div>

<style>
  /* Group wrapper: semi-transparent body fill + rounded bottom corners.
     Top corners are squared so the header strip (positioned above) connects flush.
     overflow:visible lets the header float above without being clipped.
     padding-bottom creates visual breathing room below child nodes
     without affecting their transform-based positions. */
  :global(.svelte-flow__node-group) {
    background: rgba(168, 168, 168, 0.08);
    border: 1.5px solid rgba(168, 168, 168, 0.25);
    border-radius: 0 0 var(--node-radius, 6px) var(--node-radius, 6px);
    overflow: visible;
    padding-bottom: 14px;
  }

  /* Selected: highlight border only, keep the same fill */
  :global(.svelte-flow__node-group.selected) {
    border-color: rgba(220, 220, 220, 0.75);
    box-shadow: none !important; /* suppress default xyflow selection glow */
  }

  /* Header band — floats above the group box via negative top offset.
     left/right: -1.5px compensates for the 1.5px group border so borders align.
     Height must stay in sync with GROUP_HEADER in NodeEditor.svelte (36px). */
  .group-label-wrap {
    position: absolute;
    top: -36px;
    left: -1.5px;
    right: -1.5px;
    height: 36px;
    padding: 0 12px;
    display: flex;
    align-items: center;
    background: var(--node-head-bg, #3d3d3d);
    border: 1.5px solid rgba(168, 168, 168, 0.25);
    border-bottom: none;
    border-radius: var(--node-radius, 6px) var(--node-radius, 6px) 0 0;
  }

  .group-label-wrap.selected {
    border-color: rgba(220, 220, 220, 0.75);
  }

  .group-label {
    font-family: var(--text-node-head-family);
    font-size: var(--text-node-head-size);
    font-weight: var(--text-node-head-weight);
    text-transform: var(--text-node-head-transform);
    letter-spacing: var(--text-node-head-spacing);
    color: var(--node-text, #e2e2e8);
    user-select: none;
    cursor: grab;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .group-label-input {
    background: none;
    border: none;
    border-bottom: 1px solid rgba(168, 168, 168, 0.35);
    color: var(--node-text, #e2e2e8);
    font-family: var(--text-node-head-family);
    font-size: var(--text-node-head-size);
    font-weight: var(--text-node-head-weight);
    outline: none;
    width: 100%;
    padding: 0 0 2px;
    box-sizing: border-box;
  }
</style>
