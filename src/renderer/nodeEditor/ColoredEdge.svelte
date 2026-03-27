<script lang="ts">
  import { getBezierPath, BaseEdge, type EdgeProps } from '@xyflow/svelte'

  let {
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    style = '',
    selected = false,
  }: EdgeProps = $props()

  const [edgePath] = $derived(
    getBezierPath({ sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition })
  )

  const baseColor  = $derived(style?.match(/stroke:\s*([^;]+)/)?.[1]?.trim() || '#6b7280')
  const strokeW    = $derived(selected ? 3 : 2)
  const edgeFilter = $derived(selected ? `drop-shadow(0 0 5px ${baseColor})` : 'none')
</script>

<BaseEdge
  path={edgePath}
  style="stroke: {baseColor}; stroke-width: {strokeW}; filter: {edgeFilter}"
/>
