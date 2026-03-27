import { mount } from 'svelte'
import App from './App.svelte'
import './assets/fonts.css'
import './assets/theme.css'
import { IS_ELECTRON } from './platform.js'
import { installBrowserIpc } from './browserIpc.js'

// Install the browser shim BEFORE mounting so every component sees
// window.ipcRenderer regardless of whether Electron is present.
if (!IS_ELECTRON) installBrowserIpc()

const app = mount(App, { target: document.getElementById('app')! })

export default app
