<script lang="ts">
  import ColorPicker from '../components/ColorPicker.svelte';
  import Dropdown from '../components/Dropdown.svelte';
  import ConfirmModal from '../components/ConfirmModal.svelte';
  import AboutModal from '../components/AboutModal.svelte';
  import UpdateModal, { type UpdateState } from '../components/UpdateModal.svelte';
  import RunWorkflowDialog from '../components/RunWorkflowDialog.svelte';
  import CreditsModal from '../components/CreditsModal.svelte';
  import BatchSummaryModal from '../components/BatchSummaryModal.svelte';
  import ImportProgressModal from '../components/ImportProgressModal.svelte';
  import { graphStore } from '../stores/graph.svelte.js';
  import { imageStore } from '../stores/images.svelte.js';

  // Seed mock state for store-dependent modals (showcase-only, separate renderer process)
  graphStore.batchSummary = {
    processed: 24,
    skipped: 2,
    failed: 1,
    errors: ['corrupted_scan.jpg: decode error — unsupported colour space'],
    outputDir: null,
  };
  graphStore.batchElapsedMs = 4230;
  graphStore.batchDone = true;

  imageStore.importDone = true;
  imageStore.lastImportMs = 1850;
  imageStore.lastImportCount = 27;

  let colorVal = $state([0.9, 0.35, 0.08, 1]);
  let dropVal = $state('bilinear');
  const DROP_OPTS = ['nearest', 'bilinear', 'bicubic', 'lanczos'];
  const DROP_LABELS = ['Nearest Neighbour', 'Bilinear', 'Bicubic', 'Lanczos'];

  const MOCK_STATUSES = [
    { nodeId: 'n1', label: 'Image Output 1', type: 'imageOutputNode' as const, valid: true, reasons: [] },
    {
      nodeId: 'n2',
      label: 'Image Output 2',
      type: 'imageOutputNode' as const,
      valid: false,
      reasons: ['No images loaded for connected Input node'],
    },
  ];

  const UPDATE_STATE: UpdateState = {
    status: 'latest',
    version: '1.4.0',
    body: 'You are running the latest version.',
    url: '',
  };

  // ── Design token data ──────────────────────────────────────────────────────
  const SURFACES = [
    { name: '--bg', value: '#141414', label: 'Main window / canvas background' },
    { name: '--gap-color', value: '#2b2b2b', label: 'Gap between panels (body fill)' },
    { name: '--panel-header-bg', value: '#1c1c1c', label: 'Panel title bars' },
    { name: '--search-bg', value: '#0e0e0e', label: 'Search / filter inputs' },
    { name: '--preview-bg', value: '#000000', label: 'Preview pane' },
    { name: '--node-bg', value: '#212121', label: 'Node card background' },
    { name: '--node-head-bg', value: '#3d3d3d', label: 'Node header strip' },
    { name: '--ctx-bg', value: 'var(--panel-header-bg)', label: 'Context menu / modal background' },
  ];

  const CHROMATIC = [
    { name: '--text', value: '#a8a8a8', label: 'Body / default text' },
    { name: '--text-bright', value: '#ffffff', label: 'Emphasis text, labels' },
    { name: '--border', value: '#585858', label: 'Panel borders, input outlines' },
    { name: '--accent', value: 'var(--text)', label: 'Focus rings, selected states' },
    { name: '--handle', value: '#292929', label: 'Resize handle resting' },
    { name: '--handle-hot', value: 'var(--border)', label: 'Resize handle hover/drag' },
    { name: '--modal-overlay-bg', value: 'rgba(0,0,0,0.65)', label: 'Modal backdrop overlay' },
  ];

  const STATUS_COLORS = [
    { name: '--color-success', value: '#22c55e', label: 'Success — primary' },
    { name: '--color-success-text', value: '#86efac', label: 'Success — text on dark bg' },
    { name: '--color-success-muted', value: '#81c784', label: 'Success — muted text' },
    { name: '--color-error', value: '#f87171', label: 'Error — primary' },
    { name: '--color-error-text', value: '#ff9090', label: 'Error — text on dark bg' },
    { name: '--color-error-border', value: '#7a2020', label: 'Error — list border' },
    { name: '--color-warning', value: '#f59e0b', label: 'Warning — primary' },
    { name: '--color-warning-text', value: '#fbbf24', label: 'Warning — text on dark bg' },
    { name: '--color-danger', value: '#c0392b', label: 'Danger — destructive action' },
    { name: '--color-danger-text', value: '#fca5a5', label: 'Danger — text on dark bg' },
  ];

  const PORT_COLORS = [
    { name: '--port-color-image', value: '#ff8c3f', label: 'Image' },
    { name: '--port-color-mask', value: '#d8a4fc', label: 'Mask / alpha' },
    { name: '--port-color-number', value: '#22d3ee', label: 'Number' },
    { name: '--port-color-string', value: '#22c55e', label: 'String' },
    { name: '--port-color-boolean', value: '#eab308', label: 'Boolean' },
    { name: '--port-color-color', value: '#fc86bc', label: 'Color' },
    { name: '--port-color-vector2', value: '#fb923c', label: 'Vector2' },
    { name: '--port-color-vector3', value: '#a5b4fc', label: 'Vector3' },
    { name: '--port-color-vector4', value: '#2dd4bf', label: 'Vector4' },
    { name: '--port-color-numeric', value: '#94a3b8', label: 'Numeric (poly)' },
    { name: '--port-color-any', value: '#ffffff', label: 'Any / wildcard' },
    { name: '--port-color-path', value: '#86efac', label: 'Folder path' },
  ];

  const NODE_TYPES = [
    {
      label: 'inputNode',
      accent: 'var(--node-accent-input)',
      pct: 18,
      varName: '--node-accent-input',
      file: 'src/renderer/nodeEditor/InputNode.svelte',
    },
    {
      label: 'imageOutputNode',
      accent: 'var(--node-accent-image-output)',
      pct: 18,
      varName: '--node-accent-image-output',
      file: 'src/renderer/nodeEditor/ImageOutputNode.svelte',
    },
    {
      label: 'textOutputNode',
      accent: 'var(--node-accent-text-output)',
      pct: 18,
      varName: '--node-accent-text-output',
      file: 'src/renderer/nodeEditor/TextOutputNode.svelte',
    },
    {
      label: 'flipbookOutputNode',
      accent: 'var(--node-accent-flipbook-output)',
      pct: 18,
      varName: '--node-accent-flipbook-output',
      file: 'src/renderer/nodeEditor/FlipbookOutputNode.svelte',
    },
    {
      label: 'folderPathNode',
      accent: 'var(--node-accent-folder-path)',
      pct: 18,
      varName: '--node-accent-folder-path',
      file: 'src/renderer/nodeEditor/FolderPathNode.svelte',
    },
    {
      label: 'setInputNode',
      accent: 'var(--node-accent-set-input)',
      pct: 20,
      varName: '--node-accent-set-input',
      file: 'src/renderer/nodeEditor/SetInputNode.svelte',
    },
  ];
</script>

<div class="sc-root">
  <header class="sc-page-header">
    <h1 class="sc-page-title">imgplex UI Showcase</h1>
    <p class="sc-page-sub">Visual reference for all design system components. Dev mode only.</p>
  </header>

  <!-- ── DESIGN TOKENS ──────────────────────────────────────────────────────── -->
  <section class="sc-section">
    <h2 class="sc-section-title">Design Tokens</h2>
    <p class="sc-file">src/renderer/assets/theme.css</p>

    <h3 class="sc-sub">Backgrounds &amp; Surfaces</h3>
    <div class="swatch-grid">
      {#each SURFACES as s (s.name)}
        <div class="swatch-item">
          <div class="swatch" style="background: {s.value}"></div>
          <div class="swatch-info">
            <span class="swatch-name">{s.name}</span>
            <span class="swatch-val">{s.value}</span>
            <span class="swatch-label">{s.label}</span>
          </div>
        </div>
      {/each}
    </div>

    <h3 class="sc-sub">Text, Borders &amp; Accents</h3>
    <div class="swatch-grid">
      {#each CHROMATIC as s (s.name)}
        <div class="swatch-item">
          <div class="swatch" style="background: {s.value}"></div>
          <div class="swatch-info">
            <span class="swatch-name">{s.name}</span>
            <span class="swatch-val">{s.value}</span>
            <span class="swatch-label">{s.label}</span>
          </div>
        </div>
      {/each}
    </div>

    <h3 class="sc-sub">Semantic Status Colors</h3>
    <div class="swatch-grid">
      {#each STATUS_COLORS as s (s.name)}
        <div class="swatch-item">
          <div class="swatch" style="background: {s.value}"></div>
          <div class="swatch-info">
            <span class="swatch-name">{s.name}</span>
            <span class="swatch-val">{s.value}</span>
            <span class="swatch-label">{s.label}</span>
          </div>
        </div>
      {/each}
    </div>

    <h3 class="sc-sub">Port / Edge Colors</h3>
    <div class="swatch-grid swatch-grid--ports">
      {#each PORT_COLORS as s (s.name)}
        <div class="swatch-item">
          <div class="swatch swatch--port" style="background: {s.value}"></div>
          <div class="swatch-info">
            <span class="swatch-name">{s.name}</span>
            <span class="swatch-val">{s.value}</span>
            <span class="swatch-label">{s.label}</span>
          </div>
        </div>
      {/each}
    </div>
  </section>

  <!-- ── TYPOGRAPHY ─────────────────────────────────────────────────────────── -->
  <section class="sc-section">
    <h2 class="sc-section-title">Typography</h2>
    <p class="sc-file">src/renderer/assets/theme.css · src/renderer/assets/fonts.css</p>

    <div class="type-row">
      <span class="type-meta">--font-ui · AtkinsonHyperlegibleNext</span>
      <span class="type-sample" style="font-family: var(--font-ui); font-size: var(--font-size-base)"
        >13px — Default body text. Panel labels. Inspector fields.</span
      >
      <span class="type-sample" style="font-family: var(--font-ui); font-size: 14px"
        >14px — Slightly larger UI labels.</span
      >
      <span class="type-sample" style="font-family: var(--font-ui); font-size: var(--font-size-sm); font-weight: 600"
        >12px 600 — Panel section headers. Node library categories.</span
      >
    </div>
    <div class="type-row">
      <span class="type-meta">--font-mono · JetBrainsMono</span>
      <span class="type-sample" style="font-family: var(--font-mono); font-size: var(--font-size-sm)"
        >12px — Node card headers. Computed values.</span
      >
      <span class="type-sample" style="font-family: var(--font-mono); font-size: var(--font-size-xs)"
        >11px — Port type tags. File names. Small labels.</span
      >
      <span class="type-sample" style="font-family: var(--font-mono); font-size: 14px">14px — Zoom level overlay.</span>
    </div>
  </section>

  <!-- ── BUTTONS ────────────────────────────────────────────────────────────── -->
  <section class="sc-section">
    <h2 class="sc-section-title">Buttons</h2>
    <p class="sc-file">src/renderer/assets/theme.css · .btn .btn--primary .btn--danger .btn--neutral .btn--full</p>

    <div class="row-group">
      <span class="row-label">Primary</span>
      <div class="btn-row">
        <button class="btn btn--primary">Run Workflow</button>
        <button class="btn btn--primary" disabled>Run Workflow</button>
      </div>
    </div>
    <div class="row-group">
      <span class="row-label">Danger</span>
      <div class="btn-row">
        <button class="btn btn--danger">Clear All</button>
        <button class="btn btn--danger" disabled>Clear All</button>
      </div>
    </div>
    <div class="row-group">
      <span class="row-label">Neutral</span>
      <div class="btn-row">
        <button class="btn btn--neutral">Change Folder…</button>
        <button class="btn btn--neutral" disabled>Change Folder…</button>
      </div>
    </div>
    <div class="row-group">
      <span class="row-label">Full-width</span>
      <div class="btn-col">
        <button class="btn btn--primary btn--full">Import 27 Images</button>
        <button class="btn btn--neutral btn--full">Add Images…</button>
        <button class="btn btn--danger btn--full">Clear All</button>
      </div>
    </div>
  </section>

  <!-- ── FORM CONTROLS ─────────────────────────────────────────────────────── -->
  <section class="sc-section">
    <h2 class="sc-section-title">Form Controls</h2>
    <p class="sc-file">src/renderer/components/InspectorParamEditor.svelte</p>

    <div class="controls-grid">
      <div class="ctrl-group">
        <label class="ctrl-label" for="sc-text">Text Input</label>
        <input id="sc-text" type="text" class="text-input" placeholder="Enter value…" />
      </div>
      <div class="ctrl-group">
        <label class="ctrl-label" for="sc-num">Number Input</label>
        <input id="sc-num" type="number" class="number-input" value="42" />
      </div>
      <div class="ctrl-group">
        <label class="ctrl-label" for="sc-range">Slider</label>
        <div class="sc-slider-wrap">
          <input id="sc-range" type="range" class="sc-slider" min="0" max="100" value="60" />
          <input type="number" class="number-input sc-slider-val" value="60" />
        </div>
      </div>
      <div class="ctrl-group">
        <label class="ctrl-label">Checkbox</label>
        <label class="sc-check-label">
          <input type="checkbox" checked class="sc-checkbox" />
          <span>Match Case</span>
        </label>
      </div>
    </div>
  </section>

  <!-- ── DROPDOWN ──────────────────────────────────────────────────────────── -->
  <section class="sc-section">
    <h2 class="sc-section-title">Dropdown</h2>
    <p class="sc-file">src/renderer/components/Dropdown.svelte</p>
    <div class="comp-frame">
      <Dropdown value={dropVal} options={DROP_OPTS} labels={DROP_LABELS} onchange={(v) => (dropVal = v)} />
    </div>
  </section>

  <!-- ── COLOR PICKER ──────────────────────────────────────────────────────── -->
  <section class="sc-section">
    <h2 class="sc-section-title">Color Picker</h2>
    <p class="sc-file">src/renderer/components/ColorPicker.svelte</p>
    <div class="comp-frame">
      <ColorPicker value={colorVal} onchange={(v) => (colorVal = v)} />
    </div>
  </section>

  <!-- ── NODE TYPE COLORS ───────────────────────────────────────────────────── -->
  <section class="sc-section">
    <h2 class="sc-section-title">Node Type Header Colors</h2>
    <p class="sc-file">
      src/renderer/nodeEditor/[NodeType].svelte — color-mix(in srgb, &lt;accent&gt; &lt;pct&gt;%, var(--node-head-bg))
    </p>

    <div class="node-type-list">
      {#each NODE_TYPES as n (n.label)}
        <div class="node-card">
          <div
            class="node-card-header"
            style="background: color-mix(in srgb, {n.accent} {n.pct}%, var(--node-head-bg))"
          >
            <span class="node-card-label">{n.label}</span>
            <span class="node-card-accent">{n.varName} @ {n.pct}%</span>
          </div>
          <div class="node-card-body">
            <span class="node-card-file">{n.file}</span>
          </div>
        </div>
      {/each}

      <!-- Comment node — special solid colours -->
      <div class="node-card">
        <div class="node-card-header" style="background: #fde047; color: #5c3200">
          <span class="node-card-label" style="color: #5c3200">commentNode</span>
          <span class="node-card-accent" style="color: #5c3200">#fde047 (solid)</span>
        </div>
        <div class="node-card-body" style="background: #fef08a">
          <span class="node-card-file" style="color: #422006">Body: #fef08a · Selected border: #ca8a04</span>
          <span class="node-card-file" style="color: #422006">src/renderer/nodeEditor/CommentNode.svelte</span>
        </div>
      </div>

      <!-- Group node -->
      <div class="node-card">
        <div class="node-card-header" style="background: var(--node-head-bg)">
          <span class="node-card-label">groupNode</span>
          <span class="node-card-accent">neutral header</span>
        </div>
        <div
          class="node-card-body"
          style="background: rgba(168,168,168,0.08); border: 1.5px solid rgba(168,168,168,0.25)"
        >
          <span class="node-card-file">Body: rgba(168,168,168,0.08) · Border: rgba(168,168,168,0.25)</span>
          <span class="node-card-file">src/renderer/nodeEditor/GroupNode.svelte</span>
        </div>
      </div>
    </div>
  </section>

  <!-- ── MODALS ─────────────────────────────────────────────────────────────── -->
  <section class="sc-section">
    <h2 class="sc-section-title">Modals</h2>
    <p class="sc-file">src/renderer/components/[Modal].svelte — rendered without backdrop</p>

    <div class="modals-grid">
      <div class="modal-entry">
        <div class="modal-meta">
          <span class="modal-name">ConfirmModal</span>
          <span class="modal-path">src/renderer/components/ConfirmModal.svelte</span>
        </div>
        <div class="modal-wrap">
          <ConfirmModal
            message="This will delete all loaded images. Continue?"
            onConfirm={() => {}}
            onCancel={() => {}}
          />
        </div>
      </div>

      <div class="modal-entry">
        <div class="modal-meta">
          <span class="modal-name">AboutModal</span>
          <span class="modal-path">src/renderer/components/AboutModal.svelte</span>
        </div>
        <div class="modal-wrap">
          <AboutModal onClose={() => {}} />
        </div>
      </div>

      <div class="modal-entry">
        <div class="modal-meta">
          <span class="modal-name">UpdateModal — status: latest</span>
          <span class="modal-path">src/renderer/components/UpdateModal.svelte</span>
        </div>
        <div class="modal-wrap">
          <UpdateModal state={UPDATE_STATE} onClose={() => {}} />
        </div>
      </div>

      <div class="modal-entry">
        <div class="modal-meta">
          <span class="modal-name">RunWorkflowDialog</span>
          <span class="modal-path">src/renderer/components/RunWorkflowDialog.svelte</span>
        </div>
        <div class="modal-wrap">
          <RunWorkflowDialog statuses={MOCK_STATUSES} onRun={() => {}} onCancel={() => {}} />
        </div>
      </div>

      <div class="modal-entry">
        <div class="modal-meta">
          <span class="modal-name">CreditsModal</span>
          <span class="modal-path">src/renderer/components/CreditsModal.svelte</span>
        </div>
        <div class="modal-wrap">
          <CreditsModal onClose={() => {}} />
        </div>
      </div>

      <div class="modal-entry">
        <div class="modal-meta">
          <span class="modal-name">BatchSummaryModal</span>
          <span class="modal-path">src/renderer/components/BatchSummaryModal.svelte</span>
        </div>
        <div class="modal-wrap">
          <BatchSummaryModal onClose={() => {}} />
        </div>
      </div>

      <div class="modal-entry">
        <div class="modal-meta">
          <span class="modal-name">ImportProgressModal — done state</span>
          <span class="modal-path">src/renderer/components/ImportProgressModal.svelte</span>
        </div>
        <div class="modal-wrap">
          <ImportProgressModal />
        </div>
      </div>
    </div>
  </section>
</div>

<style>
  /* ── Page shell ── */
  :global(html) {
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-thumb) transparent;
  }
  :global(html::-webkit-scrollbar) {
    width: var(--scrollbar-width);
  }
  :global(html::-webkit-scrollbar-track) {
    background: transparent;
  }
  :global(html::-webkit-scrollbar-thumb) {
    background: var(--scrollbar-thumb);
    border-radius: var(--scrollbar-radius);
    transition: background 0.15s;
  }
  :global(html::-webkit-scrollbar-thumb:hover) {
    background: var(--scrollbar-thumb-hover);
  }
  :global(body) {
    background: var(--bg);
    overflow: visible;
  }

  .sc-root {
    max-width: 960px;
    margin: 0 auto;
    padding: 32px 24px 80px;
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .sc-page-header {
    padding-bottom: 28px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 8px;
  }

  .sc-page-title {
    font-family: var(--font-ui);
    font-size: 20px;
    font-weight: 600;
    color: var(--text-bright);
    margin-bottom: 6px;
  }

  .sc-page-sub {
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    color: var(--text);
  }

  /* ── Section ── */
  .sc-section {
    padding: 28px 0;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 35%, transparent);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .sc-section-title {
    font-family: var(--font-ui);
    font-size: 16px;
    font-weight: 600;
    color: var(--text-bright);
    margin: 0;
  }

  .sc-file {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--text);
    opacity: 0.55;
    margin: -10px 0 4px;
  }

  .sc-sub {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    font-weight: 600;
    color: var(--text-bright);
    opacity: 0.55;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin: 8px 0 0;
  }

  /* ── Swatches ── */
  .swatch-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }

  .swatch-grid--ports {
    grid-template-columns: repeat(4, 1fr);
  }

  .swatch-item {
    display: flex;
    gap: 8px;
    align-items: flex-start;
  }

  .swatch {
    width: 32px;
    height: 32px;
    border-radius: 4px;
    flex-shrink: 0;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .swatch--port {
    width: 20px;
    height: 20px;
    margin-top: 2px;
    border: none;
    border-radius: 50%;
  }

  .swatch-info {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .swatch-name {
    font-family: var(--font-mono);
    font-size: var(--font-size-xxs);
    color: var(--text-bright);
    white-space: nowrap;
  }

  .swatch-val {
    font-family: var(--font-mono);
    font-size: var(--font-size-xxs);
    color: var(--text);
  }

  .swatch-label {
    font-family: var(--font-ui);
    font-size: var(--font-size-xxs);
    color: var(--text);
    opacity: 0.6;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* ── Typography ── */
  .type-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
    background: var(--panel-header-bg);
    border: 1px solid color-mix(in srgb, var(--border) 40%, transparent);
    border-radius: 4px;
  }

  .type-meta {
    font-family: var(--font-mono);
    font-size: var(--font-size-xxs);
    color: var(--text);
    opacity: 0.55;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding-bottom: 6px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 30%, transparent);
    margin-bottom: 2px;
  }

  .type-sample {
    color: var(--text-bright);
    line-height: 1.5;
  }

  /* ── Buttons ── */
  .row-group {
    display: flex;
    align-items: flex-start;
    gap: 16px;
  }

  .row-label {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--text);
    opacity: 0.6;
    width: 64px;
    flex-shrink: 0;
    padding-top: 7px;
  }

  .btn-row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .btn-col {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 280px;
  }

  /* ── Form controls ── */
  .controls-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 16px;
  }

  .ctrl-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .ctrl-label {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    color: var(--text-bright);
    opacity: 0.6;
  }

  .sc-slider-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .sc-slider {
    flex: 1;
    appearance: none;
    -webkit-appearance: none;
    height: 3px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--border) 60%, transparent);
    outline: none;
    cursor: pointer;
  }

  .sc-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
    border: none;
  }

  .sc-slider-val {
    width: 52px;
    text-align: right;
    flex-shrink: 0;
  }

  .sc-check-label {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    color: var(--text-bright);
    user-select: none;
  }

  /* ── Dropdown / ColorPicker frames ── */
  .comp-frame {
    display: flex;
    align-items: flex-start;
    padding: 16px;
    background: var(--panel-header-bg);
    border: 1px solid color-mix(in srgb, var(--border) 40%, transparent);
    border-radius: 4px;
  }

  /* ── Node type colors ── */
  .node-type-list {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }

  .node-card {
    border: 1px solid var(--node-border);
    border-radius: var(--node-radius);
    overflow: hidden;
  }

  .node-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 10px;
    height: 28px;
  }

  .node-card-label {
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    color: var(--node-text);
  }

  .node-card-accent {
    font-family: var(--font-mono);
    font-size: var(--font-size-xxs);
    color: var(--node-text);
    opacity: 0.6;
  }

  .node-card-body {
    padding: 8px 10px;
    background: var(--node-bg);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .node-card-file {
    font-family: var(--font-mono);
    font-size: var(--font-size-xxs);
    color: var(--text);
    opacity: 0.6;
  }

  /* ── Modals ── */
  .modals-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 24px;
  }

  .modal-entry {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .modal-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .modal-name {
    font-family: var(--font-ui);
    font-size: var(--font-size-sm);
    font-weight: 600;
    color: var(--text-bright);
  }

  .modal-path {
    font-family: var(--font-mono);
    font-size: var(--font-size-xxs);
    color: var(--text);
    opacity: 0.55;
  }

  /* Strip the backdrop from modals rendered inside .modal-wrap */
  .modal-wrap :global(.backdrop) {
    position: static !important;
    inset: auto !important;
    background: none !important;
    z-index: auto !important;
    display: block !important;
  }
</style>
