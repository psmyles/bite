<script lang="ts">
  import { graphStore } from '../stores/graph.svelte.js'
  import type { Node } from '@xyflow/svelte'
  import { getNodeParams } from '../nodeEditor/nodeEditorHelpers.js'

  let { selectedNode }: { selectedNode: Node } = $props()

  const params = $derived(getNodeParams(selectedNode.data))

  const heading = $derived((params.heading ?? 'Comment') as string)
  const body    = $derived((params.body    ?? '')         as string)

  function onHeadingInput(e: Event) {
    graphStore.setParam(selectedNode.id, 'heading', (e.target as HTMLInputElement).value)
  }

  function onBodyInput(e: Event) {
    graphStore.setParam(selectedNode.id, 'body', (e.target as HTMLTextAreaElement).value)
  }
</script>

<div class="comment-inspector">
  <div class="field">
    <label class="field-label" for="comment-heading">Heading</label>
    <input
      id="comment-heading"
      class="field-input"
      type="text"
      value={heading}
      oninput={onHeadingInput}
      placeholder="Heading…"
      spellcheck="false"
    />
  </div>
  <div class="field field--body">
    <label class="field-label" for="comment-body">Body</label>
    <textarea
      id="comment-body"
      class="field-textarea"
      value={body}
      oninput={onBodyInput}
      placeholder="Notes…"
      spellcheck="false"
    ></textarea>
  </div>
</div>

<style>
  .comment-inspector {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 10px 12px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field--body {
    flex: 1;
  }

  .field-label {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.6;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .field-input,
  .field-textarea {
    background: var(--input-bg, rgba(255,255,255,0.06));
    border: 1px solid var(--input-border, rgba(255,255,255,0.12));
    border-radius: 3px;
    color: var(--text);
    font-family: var(--font-ui);
    font-size: 12px;
    padding: 5px 8px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
    transition: border-color 0.12s;
  }

  .field-input:focus,
  .field-textarea:focus {
    border-color: var(--accent);
  }

  .field-textarea {
    resize: vertical;
    min-height: 100px;
    line-height: 1.5;
    font-family: var(--font-ui);
  }
</style>
