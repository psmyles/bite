// Shared type for the Run Workflow validation flow. Lives in a .ts module (not the
// .svelte component) so plain `tsc` and Node-side tests can import it by name — the
// ambient `*.svelte` module declaration only exposes the default export.
export type OutputNodeStatus = {
  nodeId: string;
  label: string;
  type: 'imageOutputNode' | 'textOutputNode' | 'flipbookOutputNode';
  valid: boolean;
  reasons: string[];
};
