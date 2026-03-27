// Patches electron-builder's NSIS templates to show file details during install.
// Runs automatically via the "postinstall" npm script after every `npm install`.
const fs = require('fs')
const path = require('path')

const patches = [
  {
    file: 'node_modules/app-builder-lib/templates/nsis/common.nsh',
    from: 'ShowInstDetails nevershow',
    to:   'ShowInstDetails show',
  },
  {
    file: 'node_modules/app-builder-lib/templates/nsis/installSection.nsh',
    from: 'SetDetailsPrint none',
    to:   'SetDetailsPrint both',
  },
]

let anyChanged = false
for (const { file, from, to } of patches) {
  const abs = path.resolve(__dirname, '..', file)
  if (!fs.existsSync(abs)) { console.warn(`patch-nsis: not found – ${file}`); continue }
  const src = fs.readFileSync(abs, 'utf8')
  if (src.includes(to)) { console.log(`patch-nsis: already patched – ${file}`); continue }
  if (!src.includes(from)) { console.warn(`patch-nsis: target string not found – ${file}`); continue }
  fs.writeFileSync(abs, src.replace(from, to), 'utf8')
  console.log(`patch-nsis: patched – ${file}`)
  anyChanged = true
}
if (!anyChanged) console.log('patch-nsis: nothing to do')
