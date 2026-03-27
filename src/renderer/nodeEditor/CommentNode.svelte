<script lang="ts">
  import { untrack } from 'svelte'
  import { NodeResizer, useSvelteFlow } from '@xyflow/svelte'
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

  const { updateNode } = useSvelteFlow()

  let localHeading  = $state(untrack(() => (data.params?.heading ?? 'Comment') as string))
  let localBody     = $state(untrack(() => (data.params?.body    ?? '')         as string))
  let editing       = $state(false)
  let editFocus     = $state<'heading' | 'body'>('heading')
  let containerEl   = $state<HTMLElement | undefined>(undefined)
  let headingInput  = $state<HTMLInputElement | undefined>(undefined)
  let bodyInput     = $state<HTMLTextAreaElement | undefined>(undefined)

  // Inline style: lock both axes when explicit dimensions are known
  const containerStyle = $derived(
    width  != null && height != null ? `width: ${width}px; height: ${height}px` :
    width  != null                   ? `width: ${width}px` :
    undefined
  )

  // Sync from store when an external change arrives (undo/redo, workflow load)
  $effect(() => {
    const h = (data.params?.heading ?? 'Comment') as string
    const b = (data.params?.body    ?? '')         as string
    untrack(() => {
      if (h !== localHeading) localHeading = h
      if (b !== localBody)    localBody    = b
    })
  })

  // Focus the right input after Svelte renders the editing branch
  $effect(() => {
    if (!editing) return
    Promise.resolve().then(() => {
      if (editFocus === 'heading') headingInput?.focus()
      else bodyInput?.focus()
    })
  })

  // Exit editing on mousedown outside the node
  $effect(() => {
    if (!editing) return
    function onDocMousedown(e: MouseEvent) {
      if (containerEl && !containerEl.contains(e.target as Node)) {
        editing = false
      }
    }
    document.addEventListener('mousedown', onDocMousedown, true)
    return () => document.removeEventListener('mousedown', onDocMousedown, true)
  })

  // On exit edit: grow vertically if content overflows the current height
  $effect(() => {
    if (editing) return
    Promise.resolve().then(() => {
      if (!containerEl || !id) return
      const scrollH = containerEl.scrollHeight
      const clientH = containerEl.clientHeight
      if (scrollH > clientH) {
        updateNode(id, { height: scrollH })
      }
    })
  })

  function enterEditing(target: 'heading' | 'body', e: MouseEvent) {
    e.stopPropagation()
    editFocus = target
    editing = true
  }

  // Stop all key events from reaching the canvas while editing
  // (prevents Space → context menu, Delete → node delete, Ctrl+Z → undo graph, etc.)
  function onKeydown(e: KeyboardEvent) {
    e.stopPropagation()
    if (e.key === 'Escape') editing = false
  }
</script>

<NodeResizer minWidth={210} minHeight={80} isVisible={selected} />

<div class="comment-node" class:selected class:editing bind:this={containerEl} style={containerStyle}>
  {#if editing}
    <input
      class="nodrag comment-heading comment-heading--edit"
      type="text"
      bind:value={localHeading}
      bind:this={headingInput}
      oninput={() => graphStore.setParam(id, 'heading', localHeading)}
      onkeydown={onKeydown}
      onclick={(e) => e.stopPropagation()}
      ondblclick={(e) => e.stopPropagation()}
      placeholder="Heading…"
      spellcheck="false"
    />
    <textarea
      class="nodrag nopan comment-body comment-body--edit"
      bind:value={localBody}
      bind:this={bodyInput}
      oninput={() => graphStore.setParam(id, 'body', localBody)}
      onkeydown={onKeydown}
      onclick={(e) => e.stopPropagation()}
      ondblclick={(e) => e.stopPropagation()}
      placeholder="Notes…"
      spellcheck="false"
    ></textarea>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="comment-heading comment-heading--display"
      ondblclick={(e) => enterEditing('heading', e)}
    >{localHeading || 'Comment'}</div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="comment-body comment-body--display"
      class:placeholder={!localBody}
      ondblclick={(e) => enterEditing('body', e)}
    >{localBody || 'Double click to edit'}</div>
  {/if}
</div>

<style>
  /* ── Sticky note shell ─────────────────────────────────────────────────── */
  .comment-node {
    background: #fef08a;
    border: none;
    border-radius: 2px;
    min-width: 210px;
    min-height: 80px;
    overflow: hidden;
    position: relative;
    cursor: grab;
    box-shadow:
      3px 5px 14px rgba(0, 0, 0, 0.55),
      0  1px  3px rgba(0, 0, 0, 0.30),
      inset 0 1px 0 rgba(255, 255, 255, 0.35);
    transition: box-shadow 0.12s;
  }

  .comment-node.selected {
    box-shadow:
      3px 5px 14px rgba(0, 0, 0, 0.55),
      0  0   0  2px #ca8a04,
      0  1px  3px rgba(0, 0, 0, 0.30),
      inset 0 1px 0 rgba(255, 255, 255, 0.35);
  }

  /* Flex layout so textarea fills all available height in edit mode */
  .comment-node.editing {
    cursor: default;
    display: flex;
    flex-direction: column;
  }

  /* ── Heading ──────────────────────────────────────────────────────────── */
  .comment-heading {
    display: block;
    width: 100%;
    background: #fde047;
    border-bottom: 1px solid rgba(0, 0, 0, 0.12);
    color: #5c3200;
    font-family: var(--font-ui, sans-serif);
    font-size: 14px;
    font-weight: 700;
    letter-spacing: normal;
    padding: 7px 10px;
    box-sizing: border-box;
    flex-shrink: 0;
  }

  .comment-heading--display {
    cursor: grab;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    user-select: none;
  }

  .comment-heading--edit {
    border: none;
    outline: 2px solid #ca8a04;
    outline-offset: -2px;
    cursor: text;
  }

  .comment-heading--edit::placeholder {
    color: rgba(113, 63, 18, 0.4);
    font-weight: 400;
    letter-spacing: 0;
  }

  /* ── Body ─────────────────────────────────────────────────────────────── */
  .comment-body {
    display: block;
    width: 100%;
    background: transparent;
    color: #422006;
    font-family: var(--font-ui, sans-serif);
    font-size: 14px;
    padding: 8px 10px;
    box-sizing: border-box;
    min-height: 52px;
  }

  .comment-body--display {
    cursor: grab;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: none;
  }

  .comment-body--display.placeholder {
    color: rgba(66, 32, 6, 0.35);
    font-style: italic;
  }

  .comment-body--edit {
    flex: 1;
    border: none;
    outline: none;
    resize: none;
    cursor: text;
    overflow-wrap: break-word;
    word-break: break-word;
  }

  .comment-body--edit::placeholder {
    color: rgba(66, 32, 6, 0.35);
    font-style: italic;
  }
</style>
