// Pure graph transforms extracted from NodeEditor.svelte so the delete / duplicate
// / cli-naming logic can be unit-tested without a DOM or the SvelteFlow runtime.
// These operate on plain node/edge shapes; the component wires up the DOM guards,
// store mutations (imageStore.removeNode for deleted inputs) and history pushes.
import type { Node, Edge } from '@xyflow/svelte';

export interface DeleteResult {
  nodes: Node[];
  edges: Edge[];
  /** Input-node ids removed by this operation — caller frees their image lists. */
  deletedInputIds: string[];
  /** False when nothing was selected/deletable (caller should no-op). */
  changed: boolean;
}

/**
 * Compute the node/edge graph after deleting the current selection.
 * Deleting a group ungroupes its children (converts them to absolute position)
 * rather than removing them. `canDeleteNode` enforces the last-input/last-output guard.
 */
export function computeDeleteSelected(
  nodes: Node[],
  edges: Edge[],
  canDeleteNode: (id: string) => boolean
): DeleteResult {
  const selectedGroups = nodes.filter((n) => n.selected && n.type === 'group');
  const selectedGroupIds = new Set(selectedGroups.map((g) => g.id));
  const groupMap = new Map(selectedGroups.map((g) => [g.id, g]));

  // Non-group nodes to delete: selected, passes deletion guard, not children of a selected group
  const toDelete = new Set(
    nodes
      .filter((n) => n.selected && n.type !== 'group' && canDeleteNode(n.id))
      .filter((n) => !n.parentId || !selectedGroupIds.has(n.parentId))
      .map((n) => n.id)
  );

  const selectedEdgeIds = new Set(edges.filter((e) => e.selected).map((e) => e.id));

  if (selectedGroupIds.size === 0 && toDelete.size === 0 && selectedEdgeIds.size === 0) {
    return { nodes, edges, deletedInputIds: [], changed: false };
  }

  // Capture deleted input-node ids BEFORE filtering `nodes` — afterwards the lookup
  // can never find them. (This ordering was the fix for the keyboard-delete leak.)
  const deletedInputIds = [...toDelete].filter((id) => nodes.find((n) => n.id === id)?.type === 'inputNode');

  const newNodes = nodes
    .filter((n) => !selectedGroupIds.has(n.id) && !toDelete.has(n.id))
    .map((n) => {
      // Children of deleted groups get converted to absolute-positioned free nodes
      if (n.parentId && selectedGroupIds.has(n.parentId)) {
        const parent = groupMap.get(n.parentId)!;
        return {
          ...n,
          position: { x: n.position.x + parent.position.x, y: n.position.y + parent.position.y },
          parentId: undefined,
          extent: undefined,
        };
      }
      return n;
    });

  const newEdges = edges.filter(
    (e) => !selectedEdgeIds.has(e.id) && !toDelete.has(e.source) && !toDelete.has(e.target)
  );

  return { nodes: newNodes, edges: newEdges, deletedInputIds, changed: true };
}

/**
 * Compute the node graph after duplicating `targets` (offset by +20,+20, selected).
 * Children of any duplicated group are pulled in and re-parented to the duplicated
 * group. `ts` is a per-call timestamp seed so duplicate ids are unique and deterministic.
 */
export function computeDuplicateNodes(nodes: Node[], targets: Node[], ts: number): { nodes: Node[]; changed: boolean } {
  if (targets.length === 0) return { nodes, changed: false };

  // Include children of any selected group nodes that aren't already in targets
  const selectedGroupIds = new Set(targets.filter((n) => n.type === 'group').map((n) => n.id));
  if (selectedGroupIds.size > 0) {
    const extraChildren = nodes.filter(
      (n) => n.parentId && selectedGroupIds.has(n.parentId) && !targets.some((t) => t.id === n.id)
    );
    targets = [...targets, ...extraChildren];
  }

  const idMap = new Map(targets.map((n, i) => [n.id, `${n.id}-dup${ts}${i}`]));
  const groups: Node[] = [];
  const rest: Node[] = [];
  for (const n of targets) {
    const dup = {
      ...n,
      id: idMap.get(n.id)!,
      position: { x: n.position.x + 20, y: n.position.y + 20 },
      selected: true,
      data: { ...(n.data as object) },
      // Remap parentId to duplicated group if parent was also duplicated
      ...(n.parentId && idMap.has(n.parentId) ? { parentId: idMap.get(n.parentId) } : {}),
    } as Node;
    if (n.type === 'group') groups.push(dup);
    else rest.push(dup);
  }
  // Groups must precede their children
  return { nodes: [...nodes.map((n) => ({ ...n, selected: false })), ...groups, ...rest], changed: true };
}

export const CLI_NAME_PREFIXES: Record<string, string> = {
  inputNode: 'input',
  imageOutputNode: 'output-image',
  textOutputNode: 'output-text',
  flipbookOutputNode: 'output-flipbook',
};

/**
 * First `prefix-N` cliName not already taken by an existing node. A plain count
 * collides after a middle node is deleted (delete input-1 → next add reuses input-2).
 */
export function makeWorkflowCliName(workflowType: string, currentNodes: Node[]): string {
  const prefix = CLI_NAME_PREFIXES[workflowType] ?? workflowType;
  const used = new Set(
    currentNodes
      .map((n) => (n.data?.params as Record<string, unknown> | undefined)?.cliName as string | undefined)
      .map((c) => c?.trim())
      .filter(Boolean)
  );
  let n = 1;
  while (used.has(`${prefix}-${n}`)) n++;
  return `${prefix}-${n}`;
}
