// Pure color conversion functions — no Svelte/DOM dependencies.

export function c01(v: number): number { return Math.max(0, Math.min(1, isNaN(v) ? 0 : v)) }
export function c255(v: number): number { return Math.round(c01(v) * 255) }

export function rgbToHsv(r: number, g: number, b: number): [number, number, number] {
  const max = Math.max(r, g, b), min = Math.min(r, g, b), d = max - min
  let h = 0
  if (d > 0) {
    if (max === r)      h = (((g - b) / d) % 6 + 6) % 6
    else if (max === g) h = (b - r) / d + 2
    else                h = (r - g) / d + 4
    h *= 60
  }
  return [h, max > 0 ? d / max : 0, max]
}

export function hsvToRgb(h: number, s: number, v: number): [number, number, number] {
  const c = v * s, x = c * (1 - Math.abs(((h / 60) % 2) - 1)), m = v - c
  const lut: [number, number, number][] = [
    [c,x,0],[x,c,0],[0,c,x],[0,x,c],[x,0,c],[c,0,x],
  ]
  const [r, g, b] = lut[Math.floor(h / 60) % 6] ?? [0, 0, 0]
  return [r + m, g + m, b + m]
}

export function slin(c: number): number {
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4)
}

export function sdelin(c: number): number {
  const v = Math.max(0, c)
  return v <= 0.0031308 ? 12.92 * v : 1.055 * Math.pow(v, 1 / 2.4) - 0.055
}

export function rgbToLab(r: number, g: number, b: number): [number, number, number] {
  const X = 0.4124564*slin(r) + 0.3575761*slin(g) + 0.1804375*slin(b)
  const Y = 0.2126729*slin(r) + 0.7151522*slin(g) + 0.0721750*slin(b)
  const Z = 0.0193339*slin(r) + 0.1191920*slin(g) + 0.9503041*slin(b)
  const f = (t: number) => t > 0.008856 ? Math.cbrt(t) : (903.3 * t + 16) / 116
  const fx = f(X/0.95047), fy = f(Y), fz = f(Z/1.08883)
  return [116*fy - 16, 500*(fx - fy), 200*(fy - fz)]
}

export function labToRgb(L: number, a: number, b: number): [number, number, number] {
  const fy = (L + 16) / 116, fx = a / 500 + fy, fz = fy - b / 200
  const inv = (t: number) => t > 0.2069 ? t*t*t : (116*t - 16) / 903.3
  const X = 0.95047 * inv(fx), Y = inv(fy), Z = 1.08883 * inv(fz)
  return [
    c01(sdelin( 3.2404542*X - 1.5371385*Y - 0.4985314*Z)),
    c01(sdelin(-0.9692660*X + 1.8760108*Y + 0.0415560*Z)),
    c01(sdelin( 0.0556434*X - 0.2040259*Y + 1.0572252*Z)),
  ]
}

export function rgbToCmyk(r: number, g: number, b: number): [number, number, number, number] {
  const k = 1 - Math.max(r, g, b)
  if (k >= 1) return [0, 0, 0, 1]
  const ki = 1 - k
  return [c01((1-r-k)/ki), c01((1-g-k)/ki), c01((1-b-k)/ki), k]
}

export function cmykToRgb(c: number, m: number, y: number, k: number): [number, number, number] {
  return [c01((1-c)*(1-k)), c01((1-m)*(1-k)), c01((1-y)*(1-k))]
}
