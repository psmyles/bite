import type { NodeGraph } from './types.js';

export const EMPTY_GRAPH: NodeGraph = { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } };

// Executor IDs — must match the "executor" field in node-definition JSON files
export const EXECUTOR = {
  CHANNEL_SPLIT: 'channel_split',
  CHANNEL_MERGE: 'channel_merge',
  MEAN_VALUE: 'mean_value',
  GATE: 'gate',
  FORMAT_CONVERT: 'format_convert',
  TEXT_OUTPUT: 'text_output',
  RENAME: 'rename',
  PROP_NAME: 'prop_name',
  PROP_PATH: 'prop_path',
  PROP_SIZE: 'prop_size',
  PROP_FILETYPE: 'prop_filetype',
  PROP_DIMENSIONS: 'prop_dimensions',
  PROP_POWER_OF_TWO: 'prop_power_of_two',
  PROCESS_AS_SET: 'process_as_set',
  RESIZE: 'resize',
} as const;

// Property executors that only need cheap I/O (fs.stat + header read) — no
// ImageMagick `identify` spawn. Shared by batch-pipeline and text-output-handlers
// so the two paths never disagree on whether a magick identify is required.
export const LIGHT_META_EXECUTORS = new Set<string>([
  EXECUTOR.PROP_NAME,
  EXECUTOR.PROP_PATH,
  EXECUTOR.PROP_SIZE,
  EXECUTOR.PROP_FILETYPE,
  EXECUTOR.PROP_DIMENSIONS,
  EXECUTOR.PROP_POWER_OF_TWO,
]);

// Canonical set of image file extensions the app accepts (lower-case, no leading
// dot). Used for open dialogs and for CLI input scanning so both accept the same
// formats. Derive a dotted set with `IMAGE_EXTENSIONS.map((e) => '.' + e)`.
export const IMAGE_EXTENSIONS = [
  // Common web / display formats
  'jpg',
  'jpeg',
  'png',
  'gif',
  'webp',
  'avif',
  'svg',
  'svgz',
  'ico',
  'bmp',
  // TIFF family
  'tif',
  'tiff',
  // HEIF / Apple
  'heic',
  'heif',
  // JPEG variants
  'jp2',
  'j2k',
  'jpf',
  'jpx',
  'jxl',
  // Professional / compositing
  'psd',
  'psb',
  'exr',
  'hdr',
  'dpx',
  'cin',
  // Camera RAW
  'cr2',
  'cr3',
  'nef',
  'nrw',
  'arw',
  'dng',
  'orf',
  'raf',
  'rw2',
  'pef',
  'srw',
  'x3f',
  '3fr',
  'kdc',
  'mrw',
  'erf',
  'rwl',
  // Legacy / misc raster
  'tga',
  'pcx',
  'ppm',
  'pgm',
  'pbm',
  'pnm',
  'sgi',
  'rgb',
  'rgba',
  'miff',
  'mng',
  'jng',
  'xbm',
  'xpm',
  'xwd',
  'sun',
  'iff',
  'lbm',
  'wbmp',
  'pict',
  'pct',
  'dds',
  'fits',
  'fts',
];

// IPC channel names
export const IPC = {
  // Node registry
  REGISTRY_GET_ALL: 'registry:get-all',
  REGISTRY_UPDATED: 'registry:updated',
  // Pipeline (Phase 4)
  LOAD_IMAGES_WITH_THUMBNAILS: 'pipeline:load-images-with-thumbnails',
  LOAD_IMAGES_STREAMING_START: 'pipeline:load-images-streaming-start',
  LOAD_IMAGES_STREAMING_RESULT: 'pipeline:load-images-streaming-result',
  LOAD_IMAGES_STREAMING_CANCEL: 'pipeline:load-images-streaming-cancel',
  GENERATE_THUMBNAIL: 'pipeline:generate-thumbnail',
  EXECUTE_PREVIEW: 'pipeline:execute-preview',
  EXECUTE_BATCH: 'pipeline:execute-batch',
  EXECUTE_BATCH_PROGRESS: 'pipeline:execute-batch:progress',
  EXECUTE_BATCH_CANCEL: 'pipeline:execute-batch-cancel',
  EXPORT_CLI: 'pipeline:export-cli',
  // File dialogs
  OPEN_IMAGES_DIALOG: 'dialog:open-images',
  OPEN_FOLDER_DIALOG: 'dialog:open-folder',
  SCAN_FOLDER_DIALOG: 'dialog:scan-folder',
  SCAN_FOLDER: 'dialog:scan-folder-only',
  // Menu actions (main → renderer)
  MENU_NEW: 'menu:new',
  MENU_OPEN_WORKFLOW: 'menu:open-workflow',
  MENU_SAVE_WORKFLOW: 'menu:save-workflow',
  MENU_SAVE_WORKFLOW_AS: 'menu:save-workflow-as',
  MENU_DUPLICATE: 'menu:duplicate',
  MENU_DELETE: 'menu:delete',
  MENU_EXIT: 'menu:exit',
  MENU_ABOUT: 'menu:about',
  MENU_CREDITS: 'menu:credits',
  MENU_CHECK_FOR_UPDATES: 'menu:check-for-updates',
  MENU_RUN_WORKFLOW: 'menu:run-workflow',
  MENU_EXPORT_CLI_PS: 'menu:export-cli-ps',
  MENU_EXPORT_CLI_BASH: 'menu:export-cli-bash',
  MENU_EXPORT_CLI_CMD: 'menu:export-cli-cmd',
  // Workflow file I/O
  WORKFLOW_SAVE: 'workflow:save',
  WORKFLOW_LOAD: 'workflow:load',
  WORKFLOW_OPEN_PATH: 'workflow:open-path',
  // File association: main → renderer, carries the file path to open
  OPEN_FILE_PATH: 'app:open-file-path',
  // App lifecycle
  APP_QUIT: 'app:quit',
  UPDATE_AVAILABLE: 'app:update-available',
  CHECK_FOR_UPDATES: 'app:check-for-updates',
  SHELL_OPEN_EXTERNAL: 'shell:open-external',
  SHELL_OPEN_PATH: 'shell:open-path',
  // Performance timers
  TIMERS_SET_ENABLED: 'timers:set-enabled',
  // Flipbook Atlas node
  ATLAS_GENERATE: 'atlas:generate',
  ATLAS_BROWSE: 'atlas:browse',
  // Text Output node
  TEXT_OUTPUT_WRITE: 'text-output:write',
  TEXT_OUTPUT_WRITE_PROGRESS: 'text-output:write-progress',
  TEXT_OUTPUT_WRITE_CANCEL: 'text-output:write-cancel',
  TEXT_OUTPUT_BROWSE: 'text-output:browse',
  TEXT_OUTPUT_PREVIEW: 'text-output:preview',
  // Log viewer
  LOG_ENTRY: 'log:entry',
  LOG_GET_ENTRIES: 'log:get-entries',
  // About
  GET_APP_VERSIONS: 'app:get-versions',
} as const;
