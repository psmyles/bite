import { app, BrowserWindow, Menu, globalShortcut, dialog, shell, ipcMain } from 'electron';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import fs from 'node:fs';
import { spawn } from 'node:child_process';
import { initLogger, log, setLogWindow, getEntries } from '../src/main/logger.js';
import { NodeRegistry } from '../src/main/nodes/registry.js';
import { PipelineExecutor } from '../src/main/pipeline/executor.js';
import {
  registerRegistryHandlers,
  registerPipelineHandlers,
  registerDialogHandlers,
  registerWorkflowHandlers,
  registerShellHandlers,
  registerScanHandlers,
  registerTextOutputHandlers,
  registerAtlasHandlers,
} from '../src/main/ipc/handlers.js';
import { timings } from '../src/main/pipeline/timing.js';
import { TEMP_DIR } from '../src/main/pipeline/thumbnail-service.js';
import { spawnMagickCapture } from '../src/main/pipeline/magick-spawn.js';
import { IPC } from '../src/shared/constants.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

app.setName('imgplex');

// ── File-association helpers ──────────────────────────────────────────────────

// Extract the first .imgplex path from a process.argv array.
// In dev: argv = ['electron', 'main.js', ...user args...]  → start at index 2
// Packaged: argv = ['imgplex.exe', ...user args...]        → start at index 1
function extractImgplexPath(argv: string[]): string | null {
  const start = app.isPackaged ? 1 : 2;
  for (let i = start; i < argv.length; i++) {
    if (!argv[i].startsWith('-') && argv[i].endsWith('.imgplex')) return argv[i];
  }
  return null;
}

// File path queued before the renderer finishes loading
let pendingFilePath: string | null = extractImgplexPath(process.argv);

// macOS: OS delivers the file via this event (fires before or after ready)
app.on('open-file', (event, filePath) => {
  event.preventDefault();
  if (win?.webContents && !win.webContents.isLoading()) {
    win.webContents.send(IPC.OPEN_FILE_PATH, filePath);
  } else {
    pendingFilePath = filePath;
  }
});

// Windows/Linux: enforce a single instance so a double-click on a second file
// brings the existing window to the front and opens the workflow there.
const gotSingleInstanceLock = app.requestSingleInstanceLock();
if (!gotSingleInstanceLock) {
  app.quit();
} else {
  app.on('second-instance', (_event, argv) => {
    const fp = extractImgplexPath(argv);
    if (win) {
      if (win.isMinimized()) win.restore();
      win.focus();
      if (fp) win.webContents.send(IPC.OPEN_FILE_PATH, fp);
    }
  });
}

// macOS GUI apps don't inherit the shell PATH, so binaries installed via
// Homebrew or MacPorts aren't visible to spawn(). Prepend the common locations.
if (process.platform === 'darwin') {
  process.env.PATH = [
    '/opt/homebrew/bin', // Homebrew (Apple Silicon)
    '/usr/local/bin', // Homebrew (Intel) + manual installs
    '/opt/local/bin', // MacPorts
    process.env.PATH,
  ]
    .filter(Boolean)
    .join(':');
}

// The built directory structure
//
// ├─┬─┬ dist
// │ │ └── index.html
// │ │
// │ ├─┬ dist-electron
// │ │ ├── main.js
// │ │ └── preload.js
//
process.env.APP_ROOT = path.join(__dirname, '..');

export const VITE_DEV_SERVER_URL = process.env['VITE_DEV_SERVER_URL'];
export const MAIN_DIST = path.join(process.env.APP_ROOT, 'dist-electron');
export const RENDERER_DIST = path.join(process.env.APP_ROOT, 'dist');

process.env.VITE_PUBLIC = VITE_DEV_SERVER_URL ? path.join(process.env.APP_ROOT, 'public') : RENDERER_DIST;

// In a packaged app the asar contains no loose files; node-definitions are
// shipped as extraFiles next to the exe so users can add their own JSON nodes.
const nodeDefinitionsDir = app.isPackaged
  ? path.join(path.dirname(app.getPath('exe')), 'node-definitions')
  : path.join(process.env.APP_ROOT, 'node-definitions');
const registry = new NodeRegistry();
const executor = new PipelineExecutor();

let win: BrowserWindow | null;
let logWin: BrowserWindow | null = null;
let showcaseWin: BrowserWindow | null = null;

function openLogWindow() {
  if (logWin && !logWin.isDestroyed()) {
    logWin.focus();
    return;
  }
  logWin = new BrowserWindow({
    width: 900,
    height: 600,
    title: 'imgplex — Log',
    autoHideMenuBar: true,
    webPreferences: { preload: path.join(__dirname, 'preload.js') },
  });
  setLogWindow(logWin);
  logWin.on('closed', () => {
    setLogWindow(null);
    logWin = null;
  });
  if (VITE_DEV_SERVER_URL) {
    logWin.loadURL(`${VITE_DEV_SERVER_URL}log-viewer.html`);
  } else {
    logWin.loadFile(path.join(RENDERER_DIST, 'log-viewer.html'));
  }
}

function openShowcaseWindow() {
  if (showcaseWin && !showcaseWin.isDestroyed()) {
    showcaseWin.focus();
    return;
  }
  showcaseWin = new BrowserWindow({
    width: 1100,
    height: 800,
    title: 'imgplex — UI Showcase',
    autoHideMenuBar: true,
    webPreferences: { preload: path.join(__dirname, 'preload.js') },
  });
  showcaseWin.on('closed', () => {
    showcaseWin = null;
  });
  if (VITE_DEV_SERVER_URL) {
    showcaseWin.loadURL(`${VITE_DEV_SERVER_URL}ui-showcase.html`);
  } else {
    showcaseWin.loadFile(path.join(RENDERER_DIST, 'ui-showcase.html'));
  }
}

function buildMenu() {
  const send = (channel: string) => () => win?.webContents.send(channel);

  const template: Electron.MenuItemConstructorOptions[] = [
    {
      label: 'File',
      submenu: [
        { label: 'New', accelerator: 'CmdOrCtrl+N', click: send('menu:new') },
        { type: 'separator' },
        { label: 'Open Workflow', accelerator: 'CmdOrCtrl+O', click: send('menu:open-workflow') },
        { label: 'Save Workflow', accelerator: 'CmdOrCtrl+S', click: send('menu:save-workflow') },
        { label: 'Save Workflow As', accelerator: 'CmdOrCtrl+Shift+S', click: send('menu:save-workflow-as') },
        { type: 'separator' },
        {
          label: 'Export CLI Script',
          submenu: [
            { label: 'PowerShell', click: send('menu:export-cli-ps') },
            { label: 'Bash', click: send('menu:export-cli-bash') },
            { label: 'Windows Command Prompt', click: send('menu:export-cli-cmd') },
          ],
        },
        { type: 'separator' },
        { label: 'Exit', accelerator: 'Alt+F4', click: send('menu:exit') },
      ],
    },
    {
      label: 'Edit',
      submenu: [
        { label: 'Undo', accelerator: 'CmdOrCtrl+Z', role: 'undo' },
        { label: 'Redo', accelerator: 'CmdOrCtrl+Y', role: 'redo' },
        { type: 'separator' },
        { label: 'Cut', accelerator: 'CmdOrCtrl+X', role: 'cut' },
        { label: 'Copy', accelerator: 'CmdOrCtrl+C', role: 'copy' },
        { label: 'Paste', accelerator: 'CmdOrCtrl+V', role: 'paste' },
        { type: 'separator' },
        { label: 'Duplicate', accelerator: 'CmdOrCtrl+D', click: send('menu:duplicate') },
        { label: 'Delete', accelerator: 'Delete', click: send('menu:delete') },
        { type: 'separator' },
        { label: 'Select All', accelerator: 'CmdOrCtrl+A', role: 'selectAll' },
      ],
    },
    {
      label: 'View',
      submenu: [
        { label: 'Actual Size', accelerator: 'CmdOrCtrl+0', role: 'resetZoom' },
        { label: 'Zoom In', accelerator: 'CmdOrCtrl+Plus', role: 'zoomIn' },
        { label: 'Zoom Out', accelerator: 'CmdOrCtrl+-', role: 'zoomOut' },
        { type: 'separator' },
        { label: 'Toggle Full Screen', accelerator: 'F11', role: 'togglefullscreen' },
      ],
    },
    {
      label: 'Help',
      submenu: [
        { label: 'About', click: send('menu:about') },
        {
          label: 'Documentation',
          click: () => {
            shell.openExternal('https://github.com/psmyles/imgplex/tree/main/docs');
          },
        },
        {
          label: 'Report a bug',
          click: () => {
            shell.openExternal('https://github.com/psmyles/imgplex/issues/new');
          },
        },
        { label: 'Credits', click: send('menu:credits') },
        { type: 'separator' },
        { label: 'Check for Updates', click: send('menu:check-for-updates') },
        { type: 'separator' },
        {
          label: 'Enable Performance Timers',
          type: 'checkbox',
          checked: false,
          click(item) {
            timings.enabled = item.checked;
          },
        },
        { type: 'separator' },
        { label: 'View Log', click: () => openLogWindow() },
        ...(VITE_DEV_SERVER_URL
          ? ([
              { type: 'separator' },
              { label: 'Show All UI Elements', click: () => openShowcaseWindow() },
            ] as Electron.MenuItemConstructorOptions[])
          : []),
      ],
    },
  ];

  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
}

function createWindow() {
  win = new BrowserWindow({
    icon: path.join(process.env.VITE_PUBLIC, 'icon.png'),
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
    },
  });

  if (VITE_DEV_SERVER_URL) {
    win.loadURL(VITE_DEV_SERVER_URL);
  } else {
    win.loadFile(path.join(RENDERER_DIST, 'index.html'));
  }
}

async function showImageMagickMissingDialog() {
  const { response } = await dialog.showMessageBox(win!, {
    type: 'warning',
    title: 'ImageMagick Not Found',
    message: 'ImageMagick is not installed or not in your PATH.',
    detail: 'imgplex requires ImageMagick to process images. Download and install it, then restart imgplex.',
    buttons: ['Download ImageMagick', 'Dismiss'],
    defaultId: 0,
    cancelId: 1,
  });
  if (response === 0) {
    shell.openExternal('https://imagemagick.org/script/download.php');
  }
}

function checkImageMagick() {
  const child = spawn('magick', ['--version'], { stdio: 'ignore' });
  child.on('error', () => showImageMagickMissingDialog());
}

function compareSemver(a: string, b: string): number {
  const parse = (v: string) => v.replace(/^v/, '').split('.').map(Number);
  const [aMaj, aMin, aPat] = parse(a);
  const [bMaj, bMin, bPat] = parse(b);
  if (aMaj !== bMaj) return aMaj - bMaj;
  if (aMin !== bMin) return aMin - bMin;
  return aPat - bPat;
}

type UpdateCheckResult =
  | { status: 'update'; version: string; body: string; url: string }
  | { status: 'latest'; version: string; body: string; url: string }
  | { status: 'error' };

async function fetchLatestRelease(): Promise<UpdateCheckResult> {
  const res = await fetch('https://api.github.com/repos/psmyles/imgplex/releases/latest', {
    headers: { 'User-Agent': 'imgplex-updater' },
  });
  if (!res.ok) return { status: 'error' };
  const data = (await res.json()) as { tag_name: string; body: string; html_url: string };
  const latest = data.tag_name ?? '';
  const current = app.getVersion();
  const releaseData = {
    version: latest,
    body: data.body ?? '',
    url: data.html_url ?? 'https://github.com/psmyles/imgplex/releases',
  };
  return compareSemver(latest, current) > 0
    ? { status: 'update', ...releaseData }
    : { status: 'latest', ...releaseData };
}

async function checkForUpdates() {
  try {
    const result = await fetchLatestRelease();
    if (result.status === 'update') {
      win?.webContents.send(IPC.UPDATE_AVAILABLE, result);
    }
  } catch {
    // Network unavailable — silently skip
  }
}

app.whenReady().then(async () => {
  initLogger(app.getPath('logs'));

  // Remove orphaned batch intermediate files left by a previous crash.
  fs.readdir(TEMP_DIR, (_err, files) => {
    for (const f of files ?? []) {
      if (f.startsWith('batch_ms_')) fs.unlink(path.join(TEMP_DIR, f), () => {});
    }
  });
  const _cLog = console.log.bind(console);
  const _cWarn = console.warn.bind(console);
  const _cError = console.error.bind(console);
  console.log = (...a: unknown[]) => {
    _cLog(...a);
    log('info', ...a);
  };
  console.warn = (...a: unknown[]) => {
    _cWarn(...a);
    log('warn', ...a);
  };
  console.error = (...a: unknown[]) => {
    _cError(...a);
    log('error', ...a);
  };

  // Load node definitions before opening the window
  await registry.load(nodeDefinitionsDir);

  log('info', `imgplex ${app.getVersion()} | ${process.platform}/${process.arch}`);
  log(
    'info',
    `resourcesPath: ${(process as NodeJS.Process & { resourcesPath?: string }).resourcesPath ?? 'undefined (dev)'}`
  );
  log('info', `nodeDefinitionsDir: ${nodeDefinitionsDir} | exists: ${fs.existsSync(nodeDefinitionsDir)}`);

  // Hot-reload definitions when node-definitions folder changes
  registry.watch(nodeDefinitionsDir);

  // Register IPC handlers
  registerRegistryHandlers(registry, () => win);
  registerPipelineHandlers(registry, executor, () => win);
  registerDialogHandlers(() => win);
  registerScanHandlers();
  registerShellHandlers();
  registerTextOutputHandlers(registry, () => win);
  registerAtlasHandlers(() => win);

  ipcMain.handle(IPC.LOG_GET_ENTRIES, () => getEntries());

  ipcMain.handle(IPC.GET_APP_VERSIONS, async () => {
    const magick = await spawnMagickCapture(['--version'])
      .then((out) => out.match(/ImageMagick (\S+)/)?.[1] ?? 'unknown')
      .catch(() => 'unknown');
    return {
      magick,
      electron: process.versions.electron,
      chrome: process.versions.chrome,
      node: process.versions.node,
    };
  });

  ipcMain.handle(IPC.CHECK_FOR_UPDATES, async () => {
    try {
      return await fetchLatestRelease();
    } catch {
      return { status: 'error' };
    }
  });

  createWindow();
  buildMenu();
  registerWorkflowHandlers(() => win);

  // After the renderer is ready: check for ImageMagick, updates, and open any pending file
  win!.webContents.once('did-finish-load', () => {
    checkImageMagick();
    checkForUpdates();
    if (pendingFilePath) {
      win!.webContents.send(IPC.OPEN_FILE_PATH, pendingFilePath);
      pendingFilePath = null;
    }
  });

  // Re-register DevTools shortcuts removed with the custom menu
  globalShortcut.register('F12', () => win?.webContents.toggleDevTools());
  globalShortcut.register('CmdOrCtrl+Shift+I', () => win?.webContents.toggleDevTools());
});

app.on('will-quit', () => globalShortcut.unregisterAll());

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
    win = null;
  }
});

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) createWindow();
});

app.on('quit', () => registry.stop());
