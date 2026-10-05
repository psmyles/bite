import type { GraphEdge, GraphNode, NodeDefinition } from '../../shared/types.js';
import { applyParamWires } from './graph-utils.js';
import { computeNodeParams, type ImageMeta } from './executor-compute.js';

/**
 * Resolve a node's params for a single image:
 *   1. Merge upstream param-wire values into the node's raw params
 *   2. Inject compute_js body for inline-JS pure-value nodes
 *   3. Run computeNodeParams (pure math/logic/value eval; no-op for image nodes)
 *   4. Cache result into resolvedParams
 *
 * Returns the resolved params and whether this is an image-pipeline node.
 */
export function resolveNodeParams(
  node: GraphNode,
  def: NodeDefinition,
  edges: GraphEdge[],
  resolvedParams: Map<string, Record<string, unknown>>,
  meta?: ImageMeta
): { params: Record<string, unknown>; isImageNode: boolean } {
  const rawParams = applyParamWires(node, edges, resolvedParams);
  const isImageNode =
    def.inputs.some((p) => p.type === 'image' || p.type === 'mask') ||
    def.outputs.some((p) => p.type === 'image' || p.type === 'mask');
  const computeInput = !isImageNode && def.compute_js ? { ...rawParams, __compute_js__: def.compute_js } : rawParams;
  const params = computeNodeParams(isImageNode ? undefined : def.executor, computeInput, meta);
  resolvedParams.set(node.id, params);
  return { params, isImageNode };
}
