<script lang="ts">
  import {
    SvelteFlow,
    Controls,
    Background,
    BackgroundVariant,
    MiniMap,
    addEdge,
    type Node,
    type Edge,
    type Connection,
    type Viewport,
  } from '@xyflow/svelte';
  import { untrack, tick } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import '@xyflow/svelte/dist/style.css';
  import DropHelper from './DropHelper.svelte';
  import ProcessNode from './ProcessNode.svelte';
  import InputNode from './InputNode.svelte';
  import ImageOutputNode from './ImageOutputNode.svelte';
  import TextOutputNode from './TextOutputNode.svelte';
  import FlipbookOutputNode from './FlipbookOutputNode.svelte';
  import CommentNode from './CommentNode.svelte';
  import GroupNode from './GroupNode.svelte';
  import FolderPathNode from './FolderPathNode.svelte';
  import CompareNode from './CompareNode.svelte';
  import SetInputNode from './SetInputNode.svelte';
  import ColoredEdge from './ColoredEdge.svelte';
  import NodeContextMenu from './NodeContextMenu.svelte';
  import { portColor } from './portColors.js';
  import { isNodeEffectivelyEnabled } from './nodeEnabledState.js';
  import { nodeTypeForDef, buildNodeData, firstMatchingHandle } from './nodeEditorHelpers.js';
  import { computeDeleteSelected, computeDuplicateNodes, makeWorkflowCliName } from './graphTransforms.js';
  import {
    isValidConnection as validateConnection,
    handleToWireType,
    resolveEffectiveWireType,
  } from './connectionValidation.js';
  import { UndoRedoManager } from './undoRedoManager.js';
  import type { NodeDefinition } from '../../shared/types.js';
  import { graphStore } from '../stores/graph.svelte.js';
  import { imageStore } from '../stores/images.svelte.js';

  let { definitions }: { definitions: NodeDefinition[] } = $props();

  // ── Workflow node types (guard against deletion of last instance) ──────────
  const WORKFLOW_TYPES = new Set(['inputNode', 'imageOutputNode', 'textOutputNode', 'flipbookOutputNode']);

  // ── Synthetic NodeDefinition objects for the context menu ─────────────────
  // IDs are prefixed with '_workflow_' so onMenuSelect can detect and dispatch them.
  const WORKFLOW_DEFS: NodeDefinition[] = [
    {
      id: '_workflow_inputNode',
      label: 'Input',
      category: 'Workflow',
      description: 'Source of images for the workflow',
      inputs: [],
      outputs: [{ type: 'image', label: 'Image' }],
      params: [],
    },
    {
      id: '_workflow_imageOutputNode',
      label: 'Image Output',
      category: 'Workflow',
      description: 'Write processed images to disk',
      inputs: [{ type: 'image', label: 'Image' }],
      outputs: [],
      params: [],
    },
    {
      id: '_workflow_textOutputNode',
      label: 'Text Output',
      category: 'Workflow',
      description: 'Write text/metadata values to a file',
      inputs: [{ type: 'image', label: 'Image' }],
      outputs: [],
      params: [],
    },
    {
      id: '_workflow_flipbookOutputNode',
      label: 'Flipbook Output',
      category: 'Workflow',
      description: 'Assemble images into a flipbook atlas',
      inputs: [{ type: 'image', label: 'Image' }],
      outputs: [],
      params: [],
    },
  ];

  // Merge workflow defs at the front so "Workflow" sorts to the top
  const allDefinitions = $derived([...WORKFLOW_DEFS, ...definitions]);

  // ── Custom node / edge types ───────────────────────────────────────────────
  const nodeTypes = {
    process: ProcessNode,
    inputNode: InputNode,
    imageOutputNode: ImageOutputNode,
    textOutputNode: TextOutputNode,
    flipbookOutputNode: FlipbookOutputNode,
    commentNode: CommentNode,
    group: GroupNode,
    folderPathNode: FolderPathNode,
    compareNode: CompareNode,
    setInputNode: SetInputNode,
  };
  const edgeTypes = { colored: ColoredEdge };

  // ── Helpers ───────────────────────────────────────────────────────────────
  const NODE_W = 150;
  const NODE_H = 58; // header (~32px) + ports row (~26px)

  function edgeStyle(type: string) {
    const c = portColor(type);
    return {
      type: 'colored',
      style: `stroke: ${c}; stroke-width: 2`,
    };
  }

  // ── Graph state — local, synced bidirectionally with graphStore ───────────
  // WHY two-way sync? @xyflow/svelte owns its node/edge arrays; it cannot
  // accept a Svelte store binding. We must keep a LOCAL copy that SvelteFlow
  // mutates (drag, connect, etc.) and mirror it into graphStore for the
  // Inspector and Toolbar. Conversely, Inspector writes go to graphStore first
  // (new array reference) and must be pulled back into the local binding.
  //
  // WHY `untrack`? Without it, the push-effect reads graphStore inside a
  // reactive context, which makes it re-fire when graphStore changes — and the
  // pull-effect does the mirror image. The result is an infinite loop:
  //   SvelteFlow → nodes change → push-effect → graphStore.nodes = n
  //   → pull-effect fires → nodes = n (same ref, no-op, but still fires…)
  // `untrack` breaks the cycle: each effect only tracks ONE side and writes
  // the other side without establishing a dependency on it.
  //
  // Initialize from graphStore so seed nodes are picked up without duplication.
  let nodes: Node[] = $state.raw(graphStore.nodes);
  let edges: Edge[] = $state.raw(graphStore.edges);

  // Push local → store (SvelteFlow mutations: drag, delete, etc.)
  $effect(() => {
    const n = nodes;
    untrack(() => {
      if (n !== graphStore.nodes) graphStore.nodes = n;
      graphStore.initClean();
    });
  });
  $effect(() => {
    const e = edges;
    untrack(() => {
      if (e !== graphStore.edges) graphStore.edges = e;
    });
  });

  // Pull store → local (Inspector param changes create a new array)
  $effect(() => {
    const n = graphStore.nodes;
    untrack(() => {
      if (n !== nodes) nodes = n;
    });
  });
  $effect(() => {
    const e = graphStore.edges;
    untrack(() => {
      if (e !== edges) edges = e;
    });
  });

  // ── Text output node: keep portIds in sync with actual edge connections ──────
  // Runs whenever edges change. Removes middle unconnected ports and ensures
  // exactly one unconnected ghost port exists at the bottom of each textOutputNode.
  $effect(() => {
    const currentEdges = edges; // reactive dep — re-runs on any edge change
    untrack(() => {
      const txNodes = nodes.filter((n) => n.type === 'textOutputNode');
      if (txNodes.length === 0) return;

      let changed = false;
      const updated = nodes.map((n) => {
        if (n.type !== 'textOutputNode') return n;
        const p = (n.data.params ?? {}) as Record<string, unknown>;
        const portIds = [...((p.portIds as string[]) ?? ['txo-0'])];
        let nextPortIndex = (p.nextPortIndex as number) ?? 1;

        const connected = new Set(
          currentEdges
            .filter((e) => e.target === n.id && e.targetHandle?.startsWith('txo-'))
            .map((e) => e.targetHandle as string)
        );

        // Keep connected ports + always keep the last (ghost) port
        const filtered = portIds.filter((pid, idx) => idx === portIds.length - 1 || connected.has(pid));

        // If the ghost port itself got a wire, add a new ghost port
        if (filtered.length > 0 && connected.has(filtered[filtered.length - 1])) {
          filtered.push(`txo-${nextPortIndex}`);
          nextPortIndex++;
        }

        if (JSON.stringify(filtered) === JSON.stringify(portIds) && nextPortIndex === (p.nextPortIndex as number))
          return n;

        changed = true;
        return { ...n, data: { ...n.data, params: { ...p, portIds: filtered, nextPortIndex } } };
      });

      if (changed) nodes = updated;
    });
  });

  function onselectionchange({ nodes: sel }: { nodes: Node[] }) {
    graphStore.selectedNodeId = sel[0]?.id ?? null;
  }

  // ── Undo / Redo ───────────────────────────────────────────────────────────
  const undoRedo = new UndoRedoManager();

  function pushHistory() {
    undoRedo.push(nodes, edges);
  }
  function scheduleHistoryPush() {
    undoRedo.schedulePush(() => ({ nodes, edges }));
  }
  function undo() {
    const s = undoRedo.undo();
    if (s) {
      nodes = s.nodes;
      edges = s.edges;
    }
  }
  function redo() {
    const s = undoRedo.redo();
    if (s) {
      nodes = s.nodes;
      edges = s.edges;
    }
  }

  // Capture initial empty-canvas state so the first undo returns to blank.
  pushHistory();

  // ── Node grouping ──────────────────────────────────────────────────────────
  const GROUP_PADDING = 40;

  const groupable = $derived(nodes.some((n) => n.selected && !WORKFLOW_TYPES.has(n.type ?? '') && n.type !== 'group'));
  const ungroupable = $derived(nodes.some((n) => n.selected && n.type === 'group'));

  function groupSelection() {
    const toGroup = nodes.filter((n) => n.selected && !WORKFLOW_TYPES.has(n.type ?? '') && n.type !== 'group');
    if (toGroup.length === 0) return;

    // Bounding box around all selected nodes
    let minX = Infinity,
      minY = Infinity,
      maxX = -Infinity,
      maxY = -Infinity;
    for (const n of toGroup) {
      const w = n.width ?? NODE_W;
      const h = n.height ?? NODE_H;
      minX = Math.min(minX, n.position.x);
      minY = Math.min(minY, n.position.y);
      maxX = Math.max(maxX, n.position.x + w);
      maxY = Math.max(maxY, n.position.y + h);
    }

    const groupX = minX - GROUP_PADDING;
    const groupY = minY - GROUP_PADDING;
    const groupW = maxX - minX + 2 * GROUP_PADDING;
    const groupH = maxY - minY + 2 * GROUP_PADDING;

    const groupId = `group-${Date.now()}`;
    const groupNode: Node = {
      id: groupId,
      type: 'group',
      position: { x: groupX, y: groupY },
      width: groupW,
      height: groupH,
      zIndex: 0,
      data: { label: 'Group', definitionId: '', params: {} } as Record<string, unknown>,
    };

    const childIds = new Set(toGroup.map((n) => n.id));
    const nonChildren: Node[] = [];
    const children: Node[] = [];

    for (const n of nodes) {
      if (childIds.has(n.id)) {
        children.push({
          ...n,
          position: { x: n.position.x - groupX, y: n.position.y - groupY },
          parentId: groupId,
          extent: 'parent' as const,
          selected: false,
        });
      } else {
        nonChildren.push({ ...n, selected: false });
      }
    }

    // Group node must appear before its children in the array
    nodes = [groupNode, ...nonChildren, ...children];
    pushHistory();
  }

  function ungroupSelection() {
    const selectedGroups = nodes.filter((n) => n.selected && n.type === 'group');
    if (selectedGroups.length === 0) return;

    const groupIds = new Set(selectedGroups.map((g) => g.id));
    const groupMap = new Map(selectedGroups.map((g) => [g.id, g]));

    const updatedNodes = nodes
      .filter((n) => !groupIds.has(n.id))
      .map((n) => {
        if (!n.parentId || !groupIds.has(n.parentId)) return n;
        const parent = groupMap.get(n.parentId)!;
        return {
          ...n,
          position: { x: n.position.x + parent.position.x, y: n.position.y + parent.position.y },
          parentId: undefined,
          extent: undefined,
        };
      });

    nodes = updatedNodes;
    pushHistory();
  }

  // ── Editor element ref + last-mouse tracking (for keyboard invocation) ──────
  let editorEl = $state<HTMLElement | undefined>(undefined);
  let lastMouseX = 0;
  let lastMouseY = 0;
  function onMouseMove(e: MouseEvent) {
    lastMouseX = e.clientX;
    lastMouseY = e.clientY;
  }

  // ── Context menu ──────────────────────────────────────────────────────────
  interface WireLine {
    x1: number;
    y1: number;
    x2: number;
    y2: number;
    color: string;
  }
  interface MenuState {
    x: number;
    y: number;
    canvasPos: { x: number; y: number };
    filterType: string | null;
    wireLine: WireLine | null; // set when menu was triggered by a wire drop
  }
  let menuState: MenuState | null = $state(null);

  function openMenu(screenX: number, screenY: number, filterType: string | null = null) {
    if (!screenToCanvas) return;

    // Compute wire preview line endpoints (source handle → menu position).
    // Query the actual rendered handle element so the start point is exact regardless
    // of node height, which port was dragged, zoom level, or viewport offset.
    let wireLine: WireLine | null = null;
    if (wireSource && editorEl) {
      const handleEl = editorEl.querySelector(
        `.svelte-flow__node[data-id="${wireSource.nodeId}"] [data-handleid="${wireSource.handleId}"]`
      ) as HTMLElement | null;
      if (handleEl) {
        const hr = handleEl.getBoundingClientRect();
        wireLine = {
          x1: hr.left + hr.width / 2,
          y1: hr.top + hr.height / 2,
          x2: screenX,
          y2: screenY,
          color: portColor(wireType ?? 'image'),
        };
      }
    }

    menuState = {
      x: screenX,
      y: screenY,
      canvasPos: screenToCanvas({ x: screenX, y: screenY }),
      filterType,
      wireLine,
    };
  }

  // Draw/remove a bezier wire preview on document.body while menu is open from wire-drop
  $effect(() => {
    const line = menuState?.wireLine;
    if (!line) return;
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg') as SVGSVGElement;
    svg.style.cssText =
      'position:fixed;inset:0;width:100%;height:100%;pointer-events:none;z-index:998;overflow:visible';
    const cp = Math.max(Math.abs(line.x2 - line.x1) * 0.4, 50);
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path') as SVGPathElement;
    path.setAttribute(
      'd',
      `M ${line.x1} ${line.y1} C ${line.x1 + cp} ${line.y1} ${line.x2 - cp} ${line.y2} ${line.x2} ${line.y2}`
    );
    path.setAttribute('stroke', line.color);
    path.setAttribute('stroke-width', '2');
    path.setAttribute('stroke-dasharray', '6 4');
    path.setAttribute('fill', 'none');
    path.setAttribute('opacity', '0.8');
    svg.appendChild(path);
    document.body.appendChild(svg);
    return () => svg.remove();
  });

  function closeMenu() {
    menuState = null;
    wireSource = null;
    wireType = null;
    wireMade = false;
  }

  function onMenuSelect(def: NodeDefinition) {
    if (!menuState) return;
    const pos = menuState.canvasPos;
    // Wire-drop: top-left of new node lands on the drop point so the input handle
    // is right where the wire ended. Otherwise center the node on the spawn point.
    const position = wireSource ? { x: pos.x, y: pos.y } : { x: pos.x - NODE_W / 2, y: pos.y - NODE_H / 2 };

    // ── Workflow node (synthetic def with _workflow_ prefix) ──────────────────
    if (def.id.startsWith('_workflow_')) {
      const workflowType = def.id.slice('_workflow_'.length);
      const defaults = WORKFLOW_NODE_DEFAULTS[workflowType];
      if (!defaults) return;
      const newId = `${workflowType}-${Date.now()}`;
      const cliName = makeWorkflowCliName(workflowType, nodes);
      nodes = [
        ...nodes,
        { id: newId, type: workflowType, position, data: { ...defaults, params: { ...defaults.params, cliName } } },
      ];

      // Auto-connect for wire-drop: image input nodes have 'out-0', output nodes have 'in-0'
      if (wireSource) {
        const wt = wireType ?? 'image';
        if (wt === 'image') {
          const newHandle = workflowType === 'inputNode' ? 'out-0' : 'in-0';
          const connection =
            wireSource.handleType === 'source'
              ? { source: wireSource.nodeId, sourceHandle: wireSource.handleId, target: newId, targetHandle: newHandle }
              : {
                  source: newId,
                  sourceHandle: newHandle,
                  target: wireSource.nodeId,
                  targetHandle: wireSource.handleId,
                };
          if (workflowType !== 'inputNode' || wireSource.handleType !== 'source') {
            if (validateConnection(connection, nodes, edges)) {
              edges = addEdge({ ...connection, ...edgeStyle(wt) }, edges);
            }
          }
        }
      }
      pushHistory();
      return;
    }

    // ── Regular (JSON-defined) node ───────────────────────────────────────────
    const newId = `${def.id}-${Date.now()}`;
    const isComment = def.id === 'comment';
    nodes = [
      ...nodes,
      {
        id: newId,
        type: nodeTypeForDef(def),
        position,
        data: buildNodeData(def),
        ...(isComment ? { width: 280, height: 120 } : {}),
      },
    ];

    // If menu was triggered by a wire drop, auto-connect the edge
    if (wireSource) {
      const wt = wireType ?? 'image';
      const newSide = wireSource.handleType === 'source' ? 'in' : 'out';
      const newHandle = firstMatchingHandle(def, wt, newSide);
      if (newHandle) {
        const connection =
          wireSource.handleType === 'source'
            ? { source: wireSource.nodeId, sourceHandle: wireSource.handleId, target: newId, targetHandle: newHandle }
            : { source: newId, sourceHandle: newHandle, target: wireSource.nodeId, targetHandle: wireSource.handleId };
        if (validateConnection(connection, nodes, edges)) {
          edges = addEdge({ ...connection, ...edgeStyle(wt) }, edges);
        }
      }
    }
    pushHistory();
  }

  // ── Connections ───────────────────────────────────────────────────────────
  function onConnect(connection: Connection) {
    wireMade = true;

    const type = handleToWireType(connection.source, connection.sourceHandle ?? null, 'source', nodes);
    const style = edgeStyle(type);

    // Remove any existing edge going into the same target handle (single-input rule).
    // Also drop any auto-added plain edge SvelteFlow may have inserted for this exact connection.
    edges = edges.filter((e) => !(e.target === connection.target && e.targetHandle === connection.targetHandle));

    const exists = edges.some(
      (e) =>
        e.source === connection.source &&
        e.target === connection.target &&
        e.sourceHandle === connection.sourceHandle &&
        e.targetHandle === connection.targetHandle
    );
    if (exists) {
      edges = edges.map((e) =>
        e.source === connection.source &&
        e.target === connection.target &&
        e.sourceHandle === connection.sourceHandle &&
        e.targetHandle === connection.targetHandle
          ? { ...e, ...style }
          : e
      );
    } else {
      edges = addEdge({ ...connection, ...style }, edges);
    }
    pushHistory();
  }

  // ── Wire-drop → context menu (filtered by port type) ──────────────────────
  let wireSource: { nodeId: string; handleId: string | null; handleType: string | null } | null = null;
  let wireType: string | null = null;
  let wireMade = false;

  function onConnectStart(
    _e: MouseEvent | TouchEvent,
    params: { nodeId: string | null; handleId: string | null; handleType: string | null }
  ) {
    if (params.nodeId) {
      wireSource = { nodeId: params.nodeId, handleId: params.handleId, handleType: params.handleType };
      wireType = handleToWireType(params.nodeId, params.handleId, params.handleType, nodes);
    }
    wireMade = false;
  }

  function onConnectEnd(e: MouseEvent | TouchEvent) {
    if (!wireMade && wireSource && e instanceof MouseEvent) {
      const target = e.target as Element;
      if (!target.closest('.svelte-flow__handle') && !target.closest('.svelte-flow__node')) {
        // Resolve a more specific type if the 'any' port has sibling constraints
        const effectiveWireType =
          wireType === 'any' && wireSource
            ? resolveEffectiveWireType(wireSource.nodeId, wireSource.handleId, nodes, edges)
            : wireType;
        openMenu(e.clientX, e.clientY, effectiveWireType);
        // Keep wireSource + wireType alive so onMenuSelect can create the edge
        return;
      }
    }
    wireSource = null;
    wireType = null;
    wireMade = false;
  }

  // ── Background pattern (read from theme.css CSS vars) ─────────────────────
  function cssProp(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  const bgVariantMap: Record<string, BackgroundVariant> = {
    dots: BackgroundVariant.Dots,
    lines: BackgroundVariant.Lines,
    cross: BackgroundVariant.Cross,
  };

  // Strip surrounding quotes that CSS string values include e.g. `"dots"` → `dots`
  const bgVariant = bgVariantMap[cssProp('--graph-bg-variant').replace(/['"]/g, '')] ?? BackgroundVariant.Dots;
  const bgGap = parseFloat(cssProp('--graph-bg-gap')) || 20;
  const bgLineWidth = parseFloat(cssProp('--graph-bg-line-width')) || 1;
  const bgColor = cssProp('--graph-bg-color');
  const bgBaseColor = cssProp('--graph-bg-base-color');

  // Connection line style — changes color while dragging a wire
  const connectionLineStyle = $derived(
    wireType
      ? `stroke: ${portColor(wireType)}; stroke-width: 2; stroke-dasharray: 6 4;`
      : `stroke: var(--edge-stroke); stroke-width: 2; stroke-dasharray: 6 4;`
  );

  // ── Viewport (zoom level) ──────────────────────────────────────────────────
  let viewport: Viewport = $state({ x: 0, y: 0, zoom: 1 });
  const zoomPct = $derived(Math.round(viewport.zoom * 100) + '%');

  // ── screenToFlowPosition + setViewport + updateNodeInternals (from DropHelper) ─
  let screenToCanvas: ((pos: { x: number; y: number }) => { x: number; y: number }) | null = $state(null);
  let setViewport: ((v: Viewport) => void) | null = $state(null);
  let updateNodeInternals: ((ids: string | string[]) => void) | null = $state(null);

  // Re-measure handle positions whenever a textOutputNode's port order changes.
  // @xyflow only remeasures handles on node resize; CSS top changes need an explicit nudge.
  // IMPORTANT: use graphStore.nodes (not local `nodes`) so that only Inspector-driven portIds
  // changes trigger this effect — NOT SvelteFlow's dimension updates, which flow through the
  // local `nodes` binding and would cause an infinite loop.
  const _textOutputPortIdSig = $derived(
    graphStore.nodes
      .filter((n) => n.type === 'textOutputNode')
      .map((n) => `${n.id}:${JSON.stringify((n.data?.params as Record<string, unknown>)?.portIds ?? [])}`)
      .join('|')
  );
  $effect(() => {
    void _textOutputPortIdSig;
    const ids = untrack(() => graphStore.nodes.filter((n) => n.type === 'textOutputNode').map((n) => n.id));
    tick().then(() => {
      if (updateNodeInternals && ids.length) updateNodeInternals(ids);
    });
  });

  // Re-measure handle positions when setInputNode suffix count changes.
  $effect(() => {
    const setNodes = nodes.filter((n) => n.type === 'setInputNode');
    setNodes.forEach((n) => {
      void JSON.stringify((n.data?.params as Record<string, unknown>)?.suffixes);
    });
    const ids = setNodes.map((n) => n.id);
    tick().then(() => {
      if (updateNodeInternals && ids.length) updateNodeInternals(ids);
    });
  });

  // isNodeEffectivelyEnabled is imported from nodeEnabledState.js
  // Wrap to close over the local nodes/edges reactive arrays.
  function checkNodeEnabled(nodeId: string): boolean {
    return isNodeEffectivelyEnabled(nodeId, nodes, edges);
  }

  // ── Double-click: on node → set preview target; on canvas → reset zoom ──────
  function onDblClick(e: MouseEvent) {
    const nodeEl = (e.target as Element).closest('.svelte-flow__node');
    if (nodeEl) {
      const nodeId = nodeEl.getAttribute('data-id');
      const nodeType = nodes.find((n) => n.id === nodeId)?.type;
      if (nodeId && !WORKFLOW_TYPES.has(nodeType ?? '') && nodeType !== 'commentNode' && checkNodeEnabled(nodeId)) {
        // Toggle: double-click the current preview node again to revert to auto
        graphStore.previewNodeId = graphStore.previewNodeId === nodeId ? null : nodeId;
      }
      return;
    }
    setViewport?.({ x: viewport.x, y: viewport.y, zoom: 1 });
  }

  // ── Right-click on canvas → open context menu ─────────────────────────────
  function onContextMenu(e: MouseEvent) {
    e.preventDefault();
    const nodeEl = (e.target as Element).closest('.svelte-flow__node');
    if (nodeEl) {
      // Allow context menu on group nodes (for ungrouping)
      const nodeId = nodeEl.getAttribute('data-id');
      if (nodes.find((n) => n.id === nodeId)?.type === 'group') openMenu(e.clientX, e.clientY);
      return;
    }
    openMenu(e.clientX, e.clientY);
  }

  // ── Delete / drag-end — push history after xyflow mutates nodes/edges ──────
  function onNodeDragStop(_e: MouseEvent | TouchEvent, node: Node | undefined) {
    // Snap child nodes back below the header band if the user drags them into it
    if (node?.parentId) {
      const parent = nodes.find((n) => n.id === node.parentId);
      if (parent?.type === 'group' && node.position.y < 0) {
        nodes = nodes.map((n) => (n.id === node.id ? { ...n, position: { x: n.position.x, y: 0 } } : n));
      }
    }
    scheduleHistoryPush();
  }

  // ── Custom delete handler — xyflow's deleteKey is disabled so we own this ────
  // Deleting a group ungroupes its children (converts to absolute) rather than
  // removing them. We handle both nodes and edges here.
  function deleteSelected() {
    const active = document.activeElement;
    if (
      active &&
      (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA' || (active as HTMLElement).isContentEditable)
    )
      return;

    const result = computeDeleteSelected(nodes, edges, (id) => graphStore.canDeleteNode(id));
    if (!result.changed) return;

    nodes = result.nodes;
    edges = result.edges;

    // Free image lists for deleted input nodes
    for (const id of result.deletedInputIds) imageStore.removeNode(id);

    pushHistory();
  }

  // ── Space / Tab key while canvas is focused → open context menu ───────────
  function onKeydown(e: KeyboardEvent) {
    // Undo / Redo
    if ((e.key === 'z' || e.key === 'Z') && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      if (e.shiftKey) {
        redo();
      } else {
        undo();
      }
      return;
    }
    if ((e.key === 'y' || e.key === 'Y') && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      redo();
      return;
    }

    // Ctrl+G — group selected nodes; Ctrl+Shift+G — ungroup
    if ((e.key === 'g' || e.key === 'G') && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      if (e.shiftKey) {
        ungroupSelection();
      } else {
        groupSelection();
      }
      return;
    }

    if ((e.key === ' ' || e.key === 'Tab') && !menuState) {
      e.preventDefault();
      // Use the last known mouse position so the menu appears where the cursor is,
      // matching right-click and wire-drop behavior.
      // Fall back to canvas center if the mouse has never entered the canvas.
      if (lastMouseX || lastMouseY) {
        openMenu(lastMouseX, lastMouseY);
      } else {
        const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
        openMenu(rect.left + rect.width / 2, rect.top + rect.height / 2);
      }
    }

    // Delete / Backspace — custom handler so group deletion ungroupes children
    if (e.key === 'Delete' || e.key === 'Backspace') {
      deleteSelected();
      return;
    }

    // Ctrl/Cmd+D — duplicate selected nodes (never duplicates workflow nodes)
    if ((e.key === 'd' || e.key === 'D') && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      duplicateNodes(nodes.filter((n) => n.selected && !WORKFLOW_TYPES.has(n.type ?? '')));
    }
  }

  function duplicateNodes(targets: Node[]) {
    const result = computeDuplicateNodes(nodes, targets, Date.now());
    if (!result.changed) return;
    nodes = result.nodes;
    pushHistory();
  }

  // ── Drag-and-drop ─────────────────────────────────────────────────────────
  function onDragOver(e: DragEvent) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
  }

  // Default params for workflow nodes dropped from the library
  const WORKFLOW_NODE_DEFAULTS: Record<
    string,
    { label: string; inputs: string[]; outputs: string[]; params: Record<string, unknown> }
  > = {
    inputNode: { label: 'Input', inputs: [], outputs: ['image'], params: { thumbnailSize: 256, cliName: '' } },
    imageOutputNode: {
      label: 'Image Output',
      inputs: ['image'],
      outputs: [],
      params: { outputPath: 'source', customPath: '', overwrite: 'skip', generateLog: false, cliName: '' },
    },
    textOutputNode: {
      label: 'Text Output',
      inputs: ['image'],
      outputs: [],
      params: {
        outputPath: '',
        portIds: ['txo-0'],
        nextPortIndex: 1,
        separatorType: 'comma',
        customSeparator: '',
        generateLog: false,
        cliName: '',
      },
    },
    flipbookOutputNode: {
      label: 'Flipbook Output',
      inputs: ['image'],
      outputs: [],
      params: {
        flipbookOutputPath: '',
        cols: 4,
        rows: 4,
        cellWidth: 128,
        cellHeight: 128,
        sortBy: 'import_order',
        bgColor: [0, 0, 0, 0],
        generateLog: false,
        cliName: '',
      },
    },
  };

  function onDrop(e: DragEvent) {
    e.preventDefault();
    if (!screenToCanvas) return;

    // Workflow node dropped from the "Workflow" section of the library
    const workflowType = e.dataTransfer?.getData('application/imgplex-node-type');
    if (workflowType && WORKFLOW_NODE_DEFAULTS[workflowType]) {
      const canvasPos = screenToCanvas({ x: e.clientX, y: e.clientY });
      const position = { x: canvasPos.x - NODE_W / 2, y: canvasPos.y - NODE_H / 2 };
      const defaults = WORKFLOW_NODE_DEFAULTS[workflowType];
      const cliName = makeWorkflowCliName(workflowType, nodes);
      const newNode: Node = {
        id: `${workflowType}-${Date.now()}`,
        type: workflowType,
        position,
        data: { ...defaults, params: { ...defaults.params, cliName } },
      };
      nodes = [...nodes, newNode];
      pushHistory();
      return;
    }

    const definitionId = e.dataTransfer?.getData('application/imgplex-node-id');
    const label = e.dataTransfer?.getData('application/imgplex-node-label');
    if (!definitionId) return;

    const canvasPos = screenToCanvas({ x: e.clientX, y: e.clientY });
    const position = {
      x: canvasPos.x - NODE_W / 2,
      y: canvasPos.y - NODE_H / 2,
    };

    const def = definitions.find((d) => d.id === definitionId);
    const isComment = def?.id === 'comment';
    const newNode: Node = {
      id: `${definitionId}-${Date.now()}`,
      type: def ? nodeTypeForDef(def) : 'process',
      position,
      data: def
        ? buildNodeData(def)
        : { label: label ?? definitionId, inputs: ['image'], outputs: ['image'], definitionId, params: {} },
      ...(isComment ? { width: 280, height: 120 } : {}),
    };

    nodes = [...nodes, newNode];
    pushHistory();
  }

  // ── Sync viewport to store (for saving) ───────────────────────────────────
  $effect(() => {
    const v = viewport;
    untrack(() => {
      graphStore.viewport = v;
    });
  });

  // ── Restore viewport when requested by store (after load/new) ─────────────
  $effect(() => {
    const pv = graphStore.pendingViewport;
    const sv = setViewport;
    if (pv && sv) {
      untrack(() => {
        sv(pv);
        graphStore.pendingViewport = null;
      });
    }
  });

  // ── Menu IPC: duplicate / delete ───────────────────────────────────────────
  $effect(() => {
    function onDuplicate() {
      const selected = nodes.filter((n) => n.selected && !WORKFLOW_TYPES.has(n.type ?? ''));
      // If nothing selected, duplicate the focused/last selected node
      const targets =
        selected.length > 0
          ? selected
          : graphStore.selectedNodeId &&
              !WORKFLOW_TYPES.has(nodes.find((n) => n.id === graphStore.selectedNodeId)?.type ?? '')
            ? nodes.filter((n) => n.id === graphStore.selectedNodeId)
            : [];
      duplicateNodes(targets);
    }

    function onDelete() {
      const targetIds = new SvelteSet(
        nodes.filter((n) => n.selected && graphStore.canDeleteNode(n.id)).map((n) => n.id)
      );
      if (targetIds.size === 0 && graphStore.selectedNodeId) {
        const id = graphStore.selectedNodeId;
        if (graphStore.canDeleteNode(id)) targetIds.add(id);
      }
      if (targetIds.size === 0) return;
      for (const id of targetIds) {
        if (nodes.find((n) => n.id === id)?.type === 'inputNode') imageStore.removeNode(id);
      }
      nodes = nodes.filter((n) => !targetIds.has(n.id));
      edges = edges.filter((e) => !targetIds.has(e.source) && !targetIds.has(e.target));
      graphStore.selectedNodeId = null;
      pushHistory();
    }

    window.ipcRenderer.on('menu:duplicate', onDuplicate);
    window.ipcRenderer.on('menu:delete', onDelete);
    return () => {
      window.ipcRenderer.off('menu:duplicate', onDuplicate);
      window.ipcRenderer.off('menu:delete', onDelete);
    };
  });
</script>

<div
  class="editor-wrap"
  tabindex="-1"
  bind:this={editorEl}
  ondragover={onDragOver}
  ondrop={onDrop}
  ondblclick={onDblClick}
  oncontextmenu={onContextMenu}
  onkeydown={onKeydown}
  onmousemove={onMouseMove}
>
  <SvelteFlow
    bind:nodes
    bind:edges
    bind:viewport
    {nodeTypes}
    {edgeTypes}
    onconnect={onConnect}
    onconnectstart={onConnectStart}
    onconnectend={onConnectEnd}
    onnodedragstop={onNodeDragStop}
    onnoderesizeend={() => scheduleHistoryPush()}
    {onselectionchange}
    isValidConnection={(conn) => validateConnection(conn, nodes, edges)}
    colorMode="dark"
    zoomOnDoubleClick={false}
    proOptions={{ hideAttribution: true }}
    deleteKey={null}
    {connectionLineStyle}
  >
    <DropHelper
      onReady={(fn) => {
        screenToCanvas = fn;
      }}
      onViewportReady={(fn) => {
        setViewport = fn;
      }}
      onUpdateNodeInternalsReady={(fn) => {
        updateNodeInternals = fn;
      }}
    />
    <Controls />
    <Background variant={bgVariant} gap={bgGap} lineWidth={bgLineWidth} patternColor={bgColor} bgColor={bgBaseColor} />
    <MiniMap />
  </SvelteFlow>
  <div class="zoom-label">{zoomPct}</div>

  {#if menuState}
    <NodeContextMenu
      x={menuState.x}
      y={menuState.y}
      filterType={menuState.filterType}
      definitions={allDefinitions}
      onSelect={onMenuSelect}
      onClose={closeMenu}
      {groupable}
      {ungroupable}
      onGroupSelection={() => {
        groupSelection();
        closeMenu();
      }}
      onUngroup={() => {
        ungroupSelection();
        closeMenu();
      }}
    />
  {/if}
</div>

<style>
  .editor-wrap {
    width: 100%;
    height: 100%;
    position: relative;
    outline: none;
  }

  /* xyflow dark mode adds a border and its own background to .svelte-flow — strip both */
  :global(.svelte-flow) {
    border: none !important;
    border-radius: 0 !important;
    background: transparent !important;
    /* Override dark mode defaults that paint edges/handles white */
    --xy-edge-stroke: var(--edge-stroke);
    --xy-edge-stroke-selected: var(--edge-stroke-selected);
    --xy-handle-background-color: var(--edge-stroke);
    --xy-handle-border-color: var(--edge-stroke);
  }

  :global(.svelte-flow__minimap) {
    border-radius: var(--minimap-radius);
    overflow: hidden;
    opacity: var(--minimap-opacity);
    transition: opacity 0.2s;
  }

  :global(.svelte-flow__minimap:hover) {
    opacity: var(--minimap-opacity-hover);
  }

  :global(.svelte-flow__controls) {
    opacity: var(--controls-opacity);
    transition: opacity 0.2s;
  }

  :global(.svelte-flow__controls:hover) {
    opacity: var(--controls-opacity-hover);
  }

  .zoom-label {
    position: absolute;
    /* sit at the bottom-centre of the 200 px minimap (right: 10px) */
    bottom: var(--zoom-label-bottom);
    right: 10px;
    width: 200px;
    text-align: center;
    font-family: var(--text-zoom-label-family);
    font-size: var(--text-zoom-label-size);
    font-weight: var(--text-zoom-label-weight);
    text-transform: var(--text-zoom-label-transform);
    letter-spacing: var(--text-zoom-label-spacing);
    font-variant-numeric: tabular-nums;
    color: var(--zoom-label-color);
    pointer-events: none;
    z-index: 5;
  }
</style>
