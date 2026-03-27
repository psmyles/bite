<script lang="ts">
  /**
   * Rendered inside <SvelteFlow> so it has access to the store context.
   * Exposes screenToFlowPosition, setViewport, and updateNodeInternals to the parent via callbacks.
   */
  import { useSvelteFlow, type Viewport } from '@xyflow/svelte'

  interface Props {
    onReady:                  (fn: (pos: { x: number; y: number }) => { x: number; y: number }) => void
    onViewportReady:          (fn: (v: Viewport) => void) => void
    onUpdateNodeInternalsReady: (fn: (ids: string | string[]) => void) => void
  }
  let { onReady, onViewportReady, onUpdateNodeInternalsReady }: Props = $props()

  const { screenToFlowPosition, setViewport, updateNodeInternals } = useSvelteFlow()

  $effect(() => {
    onReady((pos) => screenToFlowPosition(pos, { snapToGrid: false }))
    onViewportReady((v) => setViewport(v))
    onUpdateNodeInternalsReady((ids) => updateNodeInternals(ids))
  })
</script>
