import type { NodeGraph } from './types.js'

export const APP_NAME = 'imgplex'
export const WORKFLOW_EXTENSION = '.workflow'
export const PREVIEW_MAX_EDGE_PX = 512
export const PREVIEW_DEBOUNCE_MS = 75
export const THUMBNAIL_SIZE_PX = 120
export const EMPTY_GRAPH: NodeGraph = { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } }

// IPC channel names
export const IPC = {
  // Node registry
  REGISTRY_GET_ALL: 'registry:get-all',
  REGISTRY_UPDATED: 'registry:updated',
  // Pipeline (Phase 4)
  LOAD_IMAGES: 'pipeline:load-images',
  LOAD_IMAGES_WITH_THUMBNAILS: 'pipeline:load-images-with-thumbnails',
  LOAD_IMAGES_STREAMING_START:  'pipeline:load-images-streaming-start',
  LOAD_IMAGES_STREAMING_RESULT: 'pipeline:load-images-streaming-result',
  LOAD_IMAGES_STREAMING_CANCEL: 'pipeline:load-images-streaming-cancel',
  GENERATE_THUMBNAIL: 'pipeline:generate-thumbnail',
  EXECUTE_PREVIEW: 'pipeline:execute-preview',
  EXECUTE_BATCH: 'pipeline:execute-batch',
  EXPORT_CLI: 'pipeline:export-cli',
  // File dialogs
  OPEN_IMAGES_DIALOG:  'dialog:open-images',
  OPEN_FOLDER_DIALOG:  'dialog:open-folder',
  SCAN_FOLDER_DIALOG:  'dialog:scan-folder',
  SCAN_FOLDER:         'dialog:scan-folder-only',
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
  MENU_EXPORT_CLI_PS:   'menu:export-cli-ps',
  MENU_EXPORT_CLI_BASH: 'menu:export-cli-bash',
  MENU_EXPORT_CLI_CMD:  'menu:export-cli-cmd',
  // Workflow file I/O
  WORKFLOW_SAVE: 'workflow:save',
  WORKFLOW_LOAD: 'workflow:load',
  WORKFLOW_OPEN_PATH: 'workflow:open-path',
  // File association: main → renderer, carries the file path to open
  OPEN_FILE_PATH: 'app:open-file-path',
  // App lifecycle
  APP_QUIT: 'app:quit',
  SHELL_OPEN_EXTERNAL: 'shell:open-external',
  SHELL_OPEN_PATH:     'shell:open-path',
  // Text Output node
  TEXT_OUTPUT_WRITE:          'text-output:write',
  TEXT_OUTPUT_WRITE_PROGRESS: 'text-output:write-progress',
  TEXT_OUTPUT_WRITE_CANCEL:   'text-output:write-cancel',
  TEXT_OUTPUT_BROWSE:         'text-output:browse',
  TEXT_OUTPUT_PREVIEW:        'text-output:preview',
} as const
