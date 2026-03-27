<script lang="ts">
  import Dropdown from './Dropdown.svelte'
  import { c01, c255, rgbToHsv, hsvToRgb, rgbToLab, labToRgb, rgbToCmyk, cmykToRgb } from './colorConversions.js'

  let {
    value,
    onchange,
    readonly = false,
  }: {
    value: number[]
    onchange: (v: number[]) => void
    readonly?: boolean
  } = $props()

  // ── Mode definitions ──────────────────────────────────────────────────────

  type ColorMode = 'RGB01' | 'RGB255' | 'HSV' | 'LAB' | 'CMYK'

  interface ChanDef { label: string; min: number; max: number; step: number; decimals: number }

  const MODE_LABELS: Record<ColorMode, string> = {
    RGB01:  'RGB 0–1',
    RGB255: 'RGB 0–255',
    HSV:    'HSV',
    LAB:    'LAB',
    CMYK:   'CMYK',
  }

  const MODE_ORDER: ColorMode[] = ['RGB01', 'RGB255', 'HSV', 'LAB', 'CMYK']

  const COLOR_CHANS: Record<ColorMode, ChanDef[]> = {
    RGB01:  [
      { label: 'R', min: 0,    max: 1,   step: 0.001, decimals: 3 },
      { label: 'G', min: 0,    max: 1,   step: 0.001, decimals: 3 },
      { label: 'B', min: 0,    max: 1,   step: 0.001, decimals: 3 },
    ],
    RGB255: [
      { label: 'R', min: 0,    max: 255, step: 1,     decimals: 0 },
      { label: 'G', min: 0,    max: 255, step: 1,     decimals: 0 },
      { label: 'B', min: 0,    max: 255, step: 1,     decimals: 0 },
    ],
    HSV: [
      { label: 'H', min: 0,    max: 360, step: 0.1,   decimals: 1 },
      { label: 'S', min: 0,    max: 1,   step: 0.001, decimals: 3 },
      { label: 'V', min: 0,    max: 1,   step: 0.001, decimals: 3 },
    ],
    LAB: [
      { label: 'L', min: 0,    max: 100, step: 0.1,   decimals: 1 },
      { label: 'a', min: -128, max: 127, step: 0.1,   decimals: 1 },
      { label: 'b', min: -128, max: 127, step: 0.1,   decimals: 1 },
    ],
    CMYK: [
      { label: 'C', min: 0, max: 1, step: 0.001, decimals: 3 },
      { label: 'M', min: 0, max: 1, step: 0.001, decimals: 3 },
      { label: 'Y', min: 0, max: 1, step: 0.001, decimals: 3 },
      { label: 'K', min: 0, max: 1, step: 0.001, decimals: 3 },
    ],
  }

  let mode = $state<ColorMode>('RGB01')

  // ── Mode conversions ──────────────────────────────────────────────────────

  /** RGB → mode display values (color channels, no alpha). HSV uses internal state. */
  function rgbToMode(m: ColorMode, r: number, g: number, b: number): number[] {
    switch (m) {
      case 'RGB01':  return [r, g, b]
      case 'RGB255': return [r*255, g*255, b*255]
      case 'HSV':    return [hue, sat, bri]  // use internal state to preserve hue stability
      case 'LAB':    return [...rgbToLab(r, g, b)]
      case 'CMYK':   return [...rgbToCmyk(r, g, b)]
    }
  }

  /** Mode display values → RGB */
  function modeToRgb(m: ColorMode, vals: number[]): [number, number, number] {
    switch (m) {
      case 'RGB01':  return [c01(vals[0]), c01(vals[1]), c01(vals[2])]
      case 'RGB255': return [c01(vals[0]/255), c01(vals[1]/255), c01(vals[2]/255)]
      case 'HSV':    return hsvToRgb(Math.max(0, Math.min(360, vals[0] || 0)), c01(vals[1]), c01(vals[2]))
      case 'LAB':    return labToRgb(vals[0] || 0, vals[1] || 0, vals[2] || 0)
      case 'CMYK':   return cmykToRgb(c01(vals[0]), c01(vals[1]), c01(vals[2]), c01(vals[3]))
    }
  }

  // ── Internal HSV state ────────────────────────────────────────────────────

  let hue = $state(0)
  let sat = $state(1)
  let bri = $state(1)
  let sqPicking  = false
  let huePicking = false

  $effect(() => {
    const [r = 0, g = 0, b = 0] = value
    if (!sqPicking && !huePicking) {
      const [h, s, v] = rgbToHsv(r, g, b)
      hue = h; sat = s; bri = v
    }
  })

  const rgb      = $derived(hsvToRgb(hue, sat, bri))
  const alpha    = $derived(value[3] ?? 1)
  const hex      = $derived(toHex(rgb[0], rgb[1], rgb[2]))
  const hueRgb   = $derived(hsvToRgb(hue, 1, 1))
  const hueCol   = $derived(`rgb(${c255(hueRgb[0])},${c255(hueRgb[1])},${c255(hueRgb[2])})`)
  const modeVals  = $derived(rgbToMode(mode, rgb[0], rgb[1], rgb[2]))
  const alphaBg   = $derived(alphaGradient())

  function toHex(r: number, g: number, b: number) {
    return '#' + [r, g, b].map(v => Math.round(c01(v) * 255).toString(16).padStart(2, '0')).join('')
  }

  function emit() { onchange([...hsvToRgb(hue, sat, bri), value[3] ?? 1]) }

  // ── Gradient square ───────────────────────────────────────────────────────

  function updateSquare(e: PointerEvent) {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    sat = c01((e.clientX - rect.left) / rect.width)
    bri = c01(1 - (e.clientY - rect.top) / rect.height)
    emit()
  }

  function sqDown(e: PointerEvent) {
    if (readonly) return
    sqPicking = true
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    updateSquare(e)
  }
  function sqMove(e: PointerEvent) { if (sqPicking) updateSquare(e) }
  function sqUp()                  { sqPicking = false }

  // ── Hue slider ────────────────────────────────────────────────────────────

  function updateHue(e: PointerEvent) {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    hue = c01((e.clientX - rect.left) / rect.width) * 360
    emit()
  }

  function hueDown(e: PointerEvent) {
    if (readonly) return
    huePicking = true
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    updateHue(e)
  }
  function hueMove(e: PointerEvent) { if (huePicking) updateHue(e) }
  function hueUp()                  { huePicking = false }

  // ── Channel editing ───────────────────────────────────────────────────────

  function setColorChannel(chIdx: number, raw: number) {
    if (readonly) return
    const v = isNaN(raw) ? 0 : raw
    if (mode === 'HSV') {
      // Edit HSV state directly to avoid precision loss
      if (chIdx === 0) hue = Math.max(0, Math.min(360, v))
      else if (chIdx === 1) sat = c01(v)
      else                  bri = c01(v)
      emit()
    } else {
      const vals = [...modeVals]
      vals[chIdx] = v
      const [r, g, b] = modeToRgb(mode, vals)
      ;[hue, sat, bri] = rgbToHsv(r, g, b)
      onchange([r, g, b, value[3] ?? 1])
    }
  }

  function setAlpha(raw: number) {
    if (readonly) return
    onchange([...rgb, c01(isNaN(raw) ? 0 : raw)])
  }

  // ── Channel slider gradients ──────────────────────────────────────────────

  function chanGradient(chIdx: number): string {
    const ch = COLOR_CHANS[mode][chIdx]
    const N = 6
    const stops: string[] = []
    const base = [...modeVals]
    for (let i = 0; i <= N; i++) {
      const v = ch.min + (i / N) * (ch.max - ch.min)
      const vals = [...base]; vals[chIdx] = v
      const [r, g, b] = mode === 'HSV' ? hsvToRgb(vals[0], vals[1], vals[2]) : modeToRgb(mode, vals)
      stops.push(`rgb(${c255(r)},${c255(g)},${c255(b)})`)
    }
    return `linear-gradient(to right,${stops.join(',')})`
  }

  function alphaGradient(): string {
    const [r, g, b] = rgb
    return `linear-gradient(to right,rgba(${c255(r)},${c255(g)},${c255(b)},0),rgb(${c255(r)},${c255(g)},${c255(b)}))`
  }

  function fmtVal(v: number, decimals: number): string {
    return decimals === 0 ? String(Math.round(v)) : v.toFixed(decimals)
  }

  // ── Hex input ─────────────────────────────────────────────────────────────

  let hexDraft   = $state('')
  let hexEditing = $state(false)

  $effect(() => { if (!hexEditing) hexDraft = hex })

  function applyHex() {
    hexEditing = false
    const m = hexDraft.replace('#', '').trim()
    if (m.length === 6) {
      const r = parseInt(m.slice(0, 2), 16) / 255
      const g = parseInt(m.slice(2, 4), 16) / 255
      const b = parseInt(m.slice(4, 6), 16) / 255
      if (!isNaN(r + g + b)) {
        ;[hue, sat, bri] = rgbToHsv(r, g, b)
        onchange([r, g, b, value[3] ?? 1])
      }
    }
  }
</script>

<div class="cpk">
  <!-- ── Gradient square ── -->
  <div
    class="sq"
    style="--hue-col:{hueCol}"
    onpointerdown={sqDown}
    onpointermove={sqMove}
    onpointerup={sqUp}
    role="presentation"
  >
    <div class="sq-white"></div>
    <div class="sq-black"></div>
    <div class="cursor" style="left:{sat*100}%;top:{(1-bri)*100}%"></div>
  </div>

  <!-- ── Hue slider ── -->
  <div
    class="hue-bar"
    onpointerdown={hueDown}
    onpointermove={hueMove}
    onpointerup={hueUp}
    role="presentation"
  >
    <div class="hue-thumb" style="left:{hue/360*100}%"></div>
  </div>

  <!-- ── Mode dropdown ── -->
  <div class="mode-row">
    <Dropdown
      value={mode}
      options={MODE_ORDER}
      labels={MODE_ORDER.map(m => MODE_LABELS[m])}
      disabled={readonly}
      onchange={(v) => mode = v as ColorMode}
    />
  </div>

  <!-- ── Color channel rows ── -->
  {#each COLOR_CHANS[mode] as ch, chIdx}
    {@const v = modeVals[chIdx] ?? 0}
    {@const bg = chanGradient(chIdx)}
    <div class="ch-row">
      <span class="ch-lbl">{ch.label}</span>
      <input
        type="range"
        class="ch-slider"
        style="--bg:{bg}"
        min={ch.min}
        max={ch.max}
        step={ch.step}
        value={v}
        oninput={(e) => setColorChannel(chIdx, +(e.target as HTMLInputElement).value)}
        disabled={readonly}
      />
      <input
        type="number"
        class="ch-num"
        min={ch.min}
        max={ch.max}
        step={ch.step}
        value={fmtVal(v, ch.decimals)}
        oninput={(e) => setColorChannel(chIdx, +(e.target as HTMLInputElement).value)}
        disabled={readonly}
      />
    </div>
  {/each}

  <!-- ── Alpha row ── -->
  <div class="ch-row">
    <span class="ch-lbl">A</span>
    <input
      type="range"
      class="ch-slider"
      style="--bg:{alphaBg}"
      min="0" max="1" step="0.001"
      value={alpha}
      oninput={(e) => setAlpha(+(e.target as HTMLInputElement).value)}
      disabled={readonly}
    />
    <input
      type="number"
      class="ch-num"
      min="0" max="1" step="0.001"
      value={alpha.toFixed(3)}
      oninput={(e) => setAlpha(+(e.target as HTMLInputElement).value)}
      disabled={readonly}
    />
  </div>

  <!-- ── Hex row ── -->
  <div class="hex-row">
    <span class="hex-lbl">Hex</span>
    <div class="hex-sw" style="background:{hex}"></div>
    <input
      type="text"
      class="hex-inp"
      value={hexEditing ? hexDraft : hex}
      onfocus={() => { hexEditing = true; hexDraft = hex }}
      oninput={(e) => { hexDraft = (e.target as HTMLInputElement).value }}
      onblur={applyHex}
      onkeydown={(e) => { if (e.key === 'Enter') applyHex() }}
      disabled={readonly}
    />
  </div>
</div>

<style>
  .cpk {
    display: flex;
    flex-direction: column;
    border-radius: 6px;
    overflow: hidden;
    user-select: none;
  }

  /* ── Gradient square ── */
  .sq {
    position: relative;
    width: 100%;
    aspect-ratio: 4 / 3;
    background-color: var(--hue-col);
    cursor: crosshair;
    flex-shrink: 0;
    overflow: hidden;
  }

  .sq-white {
    position: absolute; inset: 0;
    background: linear-gradient(to right, #fff, transparent);
    pointer-events: none;
  }

  .sq-black {
    position: absolute; inset: 0;
    background: linear-gradient(to bottom, transparent, #000);
    pointer-events: none;
  }

  .cursor {
    position: absolute;
    width: 12px; height: 12px;
    border-radius: 50%;
    border: 2px solid #fff;
    box-shadow: 0 0 0 1px rgba(0,0,0,0.5);
    transform: translate(-50%, -50%);
    pointer-events: none;
  }

  /* ── Hue slider ── */
  .hue-bar {
    position: relative;
    height: 12px;
    margin: 8px 8px 2px;
    border-radius: 6px;
    cursor: pointer;
    flex-shrink: 0;
    background: linear-gradient(to right,
      hsl(0,100%,50%), hsl(30,100%,50%), hsl(60,100%,50%),
      hsl(90,100%,50%), hsl(120,100%,50%), hsl(150,100%,50%),
      hsl(180,100%,50%), hsl(210,100%,50%), hsl(240,100%,50%),
      hsl(270,100%,50%), hsl(300,100%,50%), hsl(330,100%,50%),
      hsl(360,100%,50%)
    );
  }

  .hue-thumb {
    position: absolute; top: 50%;
    width: 14px; height: 14px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 0 0 2px rgba(0,0,0,0.35), 0 1px 4px rgba(0,0,0,0.5);
    transform: translate(-50%, -50%);
    pointer-events: none;
  }

  /* ── Mode dropdown ── */
  .mode-row { padding: 6px 8px 2px; }

  /* ── Channel rows ── */
  .ch-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px;
  }

  .ch-lbl {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.55;
    width: 12px;
    flex-shrink: 0;
    text-align: center;
  }

  .ch-slider {
    flex: 1;
    height: 6px;
    appearance: none;
    -webkit-appearance: none;
    border-radius: 3px;
    outline: none;
    cursor: pointer;
    border: none;
  }

  .ch-slider::-webkit-slider-runnable-track {
    height: 6px;
    border-radius: 3px;
    background: var(--bg);
  }

  .ch-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 12px; height: 12px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 0 0 1px rgba(0,0,0,0.35), 0 1px 3px rgba(0,0,0,0.4);
    cursor: pointer;
    margin-top: -3px;
  }

  .ch-slider::-moz-range-track {
    height: 6px;
    border-radius: 3px;
    background: var(--bg);
  }

  .ch-slider::-moz-range-thumb {
    width: 12px; height: 12px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 0 0 1px rgba(0,0,0,0.35), 0 1px 3px rgba(0,0,0,0.4);
    cursor: pointer;
    border: none;
  }

  .ch-num {
    width: 52px;
    flex-shrink: 0;
    background: var(--panel-header-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-bright);
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 2px 5px;
    text-align: right;
    outline: none;
    appearance: textfield;
    -moz-appearance: textfield;
  }

  .ch-num::-webkit-outer-spin-button,
  .ch-num::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }

  .ch-num:focus { border-color: var(--accent); }

  /* ── Hex row ── */
  .hex-row {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 8px 8px;
    border-top: 1px solid color-mix(in srgb, var(--border) 40%, transparent);
    margin-top: 4px;
  }

  .hex-lbl {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-bright);
    opacity: 0.55;
    width: 20px;
    flex-shrink: 0;
  }

  .hex-sw {
    width: 20px; height: 20px;
    border-radius: 3px;
    border: 1px solid var(--border);
    flex-shrink: 0;
  }

  .hex-inp {
    flex: 1;
    background: var(--panel-header-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-bright);
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 3px 6px;
    outline: none;
  }

  .hex-inp:focus { border-color: var(--accent); }
</style>
