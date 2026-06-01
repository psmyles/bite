import type { Connection, Node, Edge } from '@xyflow/svelte';
import { scalarTypes, wireTypesCompatible, paramTypeToWireType, paramInHandle } from './wireTypeUtils.js';

type AnyParamDef = { name: string; type: string; readonly?: boolean };

/** Infer wire type from the handle that started the drag. */
export function handleToWireType(
  nodeId: string,
  handleId: string | null,
  handleType: string | null,
  nodes: Node[]
): string {
  // Named special handles
  if (handleId === 'folder-in') return 'path';
  if (handleId === 'prefix-in') return 'string';
  if (handleId?.startsWith('suf-in-')) return 'string';
  // Text output ports
  if (handleId === 'txo-condition') return 'boolean';
  if (handleId?.startsWith('txo-')) return 'any';

  const src = nodes.find((n) => n.id === nodeId);
  const nodeData = src?.data as Record<string, unknown> | undefined;
  if (handleId?.startsWith('param-')) {
    const paramName = handleId.replace(/^param-(in|out)-/, '');
    if (paramName === '_enabled') return 'boolean';
    const pd = (nodeData?.paramDefs as Array<{ name: string; type: string }> | undefined)?.find(
      (p) => p.name === paramName
    );
    if (!pd) return 'number';
    return paramTypeToWireType(pd.type);
  }
  return handleType === 'source'
    ? ((nodeData?.outputs as string[] | undefined)?.[0] ?? 'image')
    : ((nodeData?.inputs as string[] | undefined)?.[0] ?? 'image');
}

/** BFS from `startId` following edge sources; returns true if `goalId` is reachable. */
function canReach(startId: string, goalId: string, edges: Edge[]): boolean {
  const visited = new Set<string>();
  const queue = [startId];
  while (queue.length > 0) {
    const cur = queue.shift()!;
    if (cur === goalId) return true;
    if (visited.has(cur)) continue;
    visited.add(cur);
    for (const e of edges) {
      if (e.source === cur && !visited.has(e.target)) queue.push(e.target);
    }
  }
  return false;
}

/**
 * When a port is typed 'any', look at sibling 'any' input params that are already
 * wired to determine the effective constrained wire type. Used to correctly filter
 * the context menu when wire-dropping from an 'any' port that has constraints.
 */
export function resolveEffectiveWireType(
  nodeId: string,
  handleId: string | null,
  nodes: Node[],
  edges: Edge[]
): string {
  const raw = handleToWireType(nodeId, handleId, null, nodes);
  if (raw !== 'any') return raw;
  const nodeData = nodes.find((n) => n.id === nodeId)?.data as Record<string, unknown> | undefined;
  const paramDefs = nodeData?.paramDefs as AnyParamDef[] | undefined;
  if (!paramDefs) return 'any';
  for (const p of paramDefs.filter((pd) => pd.type === 'any' && !pd.readonly)) {
    const edge = edges.find((e) => e.target === nodeId && e.targetHandle === paramInHandle(p.name));
    if (!edge) continue;
    const t = handleToWireType(edge.source, edge.sourceHandle ?? null, 'source', nodes);
    if (t !== 'any') return t;
  }
  return 'any';
}

/** channel_merge special case: scalar numeric wires can drive image inputs as gray fill 0–1. */
function isChannelMergeScalarInput(conn: Connection, srcType: string, tgtType: string, nodes: Node[]): boolean {
  return (
    tgtType === 'image' &&
    scalarTypes.has(srcType) &&
    !!conn.targetHandle?.startsWith('in-') &&
    (nodes.find((n) => n.id === conn.target)?.data as Record<string, unknown>)?.definitionId === 'channel_merge'
  );
}

/** All other 'any' inputs on the target node must carry a type compatible with srcType. */
function siblingAnyInputsCompatible(conn: Connection, srcType: string, nodes: Node[], edges: Edge[]): boolean {
  const paramDefs = (nodes.find((n) => n.id === conn.target)?.data as Record<string, unknown>)?.paramDefs as
    | AnyParamDef[]
    | undefined;
  const siblings = paramDefs?.filter(
    (p) => p.type === 'any' && !p.readonly && `param-in-${p.name}` !== conn.targetHandle
  );
  for (const sib of siblings ?? []) {
    const sibEdge = edges.find((e) => e.target === conn.target && e.targetHandle === `param-in-${sib.name}`);
    if (!sibEdge) continue;
    if (!wireTypesCompatible(srcType, handleToWireType(sibEdge.source, sibEdge.sourceHandle ?? null, 'source', nodes)))
      return false;
  }
  return true;
}

/**
 * When an 'any' input gets a concrete type, the node's 'any' outputs are constrained.
 * Verify all existing edges from those outputs remain compatible with resolvedSrc.
 * Example: Branch Result→folder already wired; connecting image→If True must be rejected.
 */
function downstreamAnyOutputsCompatible(conn: Connection, resolvedSrc: string, nodes: Node[], edges: Edge[]): boolean {
  if (resolvedSrc === 'any') return true;
  const paramDefs = (nodes.find((n) => n.id === conn.target)?.data as Record<string, unknown>)?.paramDefs as
    | AnyParamDef[]
    | undefined;
  for (const p of paramDefs?.filter((p) => p.type === 'any' && p.readonly) ?? []) {
    for (const outEdge of edges.filter((e) => e.source === conn.target && e.sourceHandle === `param-out-${p.name}`)) {
      if (
        !wireTypesCompatible(
          resolvedSrc,
          handleToWireType(outEdge.target, outEdge.targetHandle ?? null, 'target', nodes)
        )
      )
        return false;
    }
  }
  return true;
}

/** Returns true only when the connection is type-compatible, non-self, and cycle-free. */
export function isValidConnection(connection: Connection, nodes: Node[], edges: Edge[]): boolean {
  if (connection.source === connection.target) return false;
  if (canReach(connection.target, connection.source, edges)) return false;

  const srcType = handleToWireType(connection.source, connection.sourceHandle ?? null, 'source', nodes);
  const tgtType = handleToWireType(connection.target, connection.targetHandle ?? null, 'target', nodes);
  const resolvedSrc =
    srcType === 'any'
      ? resolveEffectiveWireType(connection.source, connection.sourceHandle ?? null, nodes, edges)
      : srcType;
  const resolvedTgt =
    tgtType === 'any'
      ? resolveEffectiveWireType(connection.target, connection.targetHandle ?? null, nodes, edges)
      : tgtType;

  if (!isChannelMergeScalarInput(connection, srcType, tgtType, nodes) && !wireTypesCompatible(resolvedSrc, resolvedTgt))
    return false;
  if (tgtType === 'any' && !siblingAnyInputsCompatible(connection, srcType, nodes, edges)) return false;
  if (tgtType === 'any' && !downstreamAnyOutputsCompatible(connection, resolvedSrc, nodes, edges)) return false;

  return true;
}
