import { describe, it, expect } from 'vitest'
import {
  numericRaw, numericAdd, numericSub, numericMul, numericDiv,
  numericPow, numericLerp, numericTruthy, numericScalar,
  computeNodeParams, buildResizeArgs,
  type ImageMeta,
} from '../main/pipeline/executor-compute.js'

// ── numericRaw ────────────────────────────────────────────────────────────────

describe('numericRaw', () => {
  it('converts number to number', () => expect(numericRaw(5)).toBe(5))
  it('converts string to number', () => expect(numericRaw('3.5')).toBe(3.5))
  it('returns 0 for null/undefined', () => {
    expect(numericRaw(null)).toBe(0)
    expect(numericRaw(undefined)).toBe(0)
  })
  it('maps array elements to numbers', () => {
    expect(numericRaw([1, '2', 3])).toEqual([1, 2, 3])
  })
})

// ── numericAdd ────────────────────────────────────────────────────────────────

describe('numericAdd', () => {
  it('scalar + scalar', () => expect(numericAdd(3, 4)).toBe(7))
  it('scalar + array (broadcast)', () => expect(numericAdd(2, [10, 20])).toEqual([12, 22]))
  it('array + scalar (broadcast)', () => expect(numericAdd([10, 20], 2)).toEqual([12, 22]))
  it('array + array (element-wise)', () => expect(numericAdd([1, 2, 3], [4, 5, 6])).toEqual([5, 7, 9]))
})

// ── numericSub ────────────────────────────────────────────────────────────────

describe('numericSub', () => {
  it('scalar - scalar', () => expect(numericSub(10, 3)).toBe(7))
  it('scalar - array (scalar is minuend)', () => expect(numericSub(10, [1, 2])).toEqual([9, 8]))
  it('array - scalar', () => expect(numericSub([10, 20], 5)).toEqual([5, 15]))
  it('array - array', () => expect(numericSub([5, 6], [1, 2])).toEqual([4, 4]))
})

// ── numericMul ────────────────────────────────────────────────────────────────

describe('numericMul', () => {
  it('scalar × scalar', () => expect(numericMul(3, 4)).toBe(12))
  it('scalar × array', () => expect(numericMul(2, [3, 4])).toEqual([6, 8]))
  it('array × scalar', () => expect(numericMul([3, 4], 2)).toEqual([6, 8]))
  it('array × array', () => expect(numericMul([2, 3], [4, 5])).toEqual([8, 15]))
})

// ── numericDiv ────────────────────────────────────────────────────────────────

describe('numericDiv', () => {
  it('scalar / scalar', () => expect(numericDiv(10, 2)).toBe(5))
  it('division by zero returns 0', () => expect(numericDiv(5, 0)).toBe(0))
  it('array / scalar', () => expect(numericDiv([6, 9], 3)).toEqual([2, 3]))
  it('array element / zero returns 0', () => expect(numericDiv([4, 6], [2, 0])).toEqual([2, 0]))
})

// ── numericPow ────────────────────────────────────────────────────────────────

describe('numericPow', () => {
  it('scalar ^ exponent', () => expect(numericPow(2, 10)).toBe(1024))
  it('array ^ exponent (element-wise)', () => expect(numericPow([2, 3], 2)).toEqual([4, 9]))
})

// ── numericLerp ───────────────────────────────────────────────────────────────

describe('numericLerp', () => {
  it('t=0 returns a', () => expect(numericLerp(0, 10, 0)).toBe(0))
  it('t=1 returns b', () => expect(numericLerp(0, 10, 1)).toBe(10))
  it('t=0.5 returns midpoint', () => expect(numericLerp(0, 10, 0.5)).toBe(5))
  it('array lerp (element-wise)', () => {
    expect(numericLerp([0, 0], [10, 20], 0.5)).toEqual([5, 10])
  })
})

// ── numericTruthy ─────────────────────────────────────────────────────────────

describe('numericTruthy', () => {
  it('non-zero scalar is truthy', () => expect(numericTruthy(1)).toBe(true))
  it('zero scalar is falsy', () => expect(numericTruthy(0)).toBe(false))
  it('array with any non-zero is truthy', () => expect(numericTruthy([0, 0, 1])).toBe(true))
  it('all-zero array is falsy', () => expect(numericTruthy([0, 0, 0])).toBe(false))
  it('false is falsy', () => expect(numericTruthy(false)).toBe(false))
  it('true is truthy', () => expect(numericTruthy(true)).toBe(true))
})

// ── numericScalar ─────────────────────────────────────────────────────────────

describe('numericScalar', () => {
  it('scalar passes through', () => expect(numericScalar(5)).toBe(5))
  it('array returns magnitude', () => {
    expect(numericScalar([3, 4])).toBeCloseTo(5) // 3-4-5 triangle
  })
  it('null/undefined returns 0', () => expect(numericScalar(null)).toBe(0))
})

// ── computeNodeParams ─────────────────────────────────────────────────────────

describe('computeNodeParams', () => {
  // ── math ──────────────────────────────────────────────────────────────────

  it('math_add: adds scalars', () => {
    const result = computeNodeParams('math_add', { a: 3, b: 7 })
    expect(result.result).toBe(10)
  })

  it('math_subtract: subtracts scalars', () => {
    const result = computeNodeParams('math_subtract', { a: 10, b: 4 })
    expect(result.result).toBe(6)
  })

  it('math_multiply: multiplies scalars', () => {
    const result = computeNodeParams('math_multiply', { a: 3, b: 4 })
    expect(result.result).toBe(12)
  })

  it('math_divide: divides scalars', () => {
    const result = computeNodeParams('math_divide', { a: 10, b: 4 })
    expect(result.result).toBe(2.5)
  })

  it('math_power: raises to exponent', () => {
    const result = computeNodeParams('math_power', { base: 2, exponent: 8 })
    expect(result.result).toBe(256)
  })

  it('math_lerp: interpolates at t=0.5', () => {
    const result = computeNodeParams('math_lerp', { a: 0, b: 20, t: 0.5 })
    expect(result.result).toBe(10)
  })

  // ── logic ─────────────────────────────────────────────────────────────────

  it('logic_and: true && true = true', () => {
    expect(computeNodeParams('logic_and', { a: 1, b: 1 }).result).toBe(true)
  })

  it('logic_and: true && false = false', () => {
    expect(computeNodeParams('logic_and', { a: 1, b: 0 }).result).toBe(false)
  })

  it('logic_or: false || true = true', () => {
    expect(computeNodeParams('logic_or', { a: 0, b: 1 }).result).toBe(true)
  })

  it('logic_not: negates truthy', () => {
    expect(computeNodeParams('logic_not', { a: 1 }).result).toBe(false)
    expect(computeNodeParams('logic_not', { a: 0 }).result).toBe(true)
  })

  it('logic_branch: selects value_true when condition is truthy', () => {
    const r = computeNodeParams('logic_branch', { condition: true, value_true: 'yes', value_false: 'no' })
    expect(r.result).toBe('yes')
  })

  it('logic_branch: selects value_false when condition is falsy', () => {
    const r = computeNodeParams('logic_branch', { condition: false, value_true: 'yes', value_false: 'no' })
    expect(r.result).toBe('no')
  })

  describe('logic_comparison', () => {
    const cmp = (op: string, a: number, b: number) =>
      computeNodeParams('logic_comparison', { operator: op, a, b }).result

    it('equal', () => {
      expect(cmp('equal', 5, 5)).toBe(true)
      expect(cmp('equal', 5, 6)).toBe(false)
    })
    it('not equal', () => {
      expect(cmp('not equal', 5, 6)).toBe(true)
      expect(cmp('not equal', 5, 5)).toBe(false)
    })
    it('greater than', () => {
      expect(cmp('greater than', 6, 5)).toBe(true)
      expect(cmp('greater than', 5, 5)).toBe(false)
    })
    it('less than', () => {
      expect(cmp('less than', 4, 5)).toBe(true)
      expect(cmp('less than', 5, 5)).toBe(false)
    })
    it('greater or equal', () => {
      expect(cmp('greater or equal', 5, 5)).toBe(true)
      expect(cmp('greater or equal', 4, 5)).toBe(false)
    })
    it('less or equal', () => {
      expect(cmp('less or equal', 5, 5)).toBe(true)
      expect(cmp('less or equal', 6, 5)).toBe(false)
    })
    it('unknown operator returns false', () => {
      expect(cmp('unknown_op', 5, 5)).toBe(false)
    })
  })

  // ── vector ────────────────────────────────────────────────────────────────

  it('split_vec: splits array into xyzw', () => {
    const r = computeNodeParams('split_vec', { vec: [1, 2, 3, 4] })
    expect(r.x).toBe(1)
    expect(r.y).toBe(2)
    expect(r.z).toBe(3)
    expect(r.w).toBe(4)
  })

  it('split_vec: scalar input maps to x, rest 0', () => {
    const r = computeNodeParams('split_vec', { vec: 7 })
    expect(r.x).toBe(7)
    expect(r.y).toBe(0)
  })

  it('append_vec: assembles vector from xyzw', () => {
    const r = computeNodeParams('append_vec', { x: 1, y: 2, z: 3, w: 4, dimensions: 4 })
    expect(r.result).toEqual([1, 2, 3, 4])
  })

  it('append_vec: respects dimensions clamp', () => {
    const r = computeNodeParams('append_vec', { x: 1, y: 2, z: 3, w: 4, dimensions: 2 })
    expect(r.result).toEqual([1, 2])
  })

  it('vec_math_dot: dot product of two arrays', () => {
    const r = computeNodeParams('vec_math_dot', { a: [1, 2, 3], b: [4, 5, 6] })
    expect(r.result).toBe(32) // 1*4 + 2*5 + 3*6
  })

  it('vec_math_length: magnitude of vector', () => {
    const r = computeNodeParams('vec_math_length', { vec: [3, 4] })
    expect(r.result).toBeCloseTo(5)
  })

  it('vec_math_normalize: unit vector', () => {
    const r = computeNodeParams('vec_math_normalize', { vec: [3, 4] })
    const result = r.result as number[]
    const len = Math.sqrt(result[0] ** 2 + result[1] ** 2)
    expect(len).toBeCloseTo(1)
  })

  it('vec_math_normalize: zero vector returns zeros', () => {
    const r = computeNodeParams('vec_math_normalize', { vec: [0, 0, 0] })
    expect(r.result).toEqual([0, 0, 0])
  })

  // ── text_filter ───────────────────────────────────────────────────────────

  it('text_filter: matches when all empty constraints', () => {
    const r = computeNodeParams('text_filter', { input: 'hello', prefix: '', suffix: '', contains: '' })
    expect(r.result).toBe(true)
  })

  it('text_filter: prefix match', () => {
    const r = computeNodeParams('text_filter', { input: 'IMG_001.jpg', prefix: 'IMG', suffix: '', contains: '' })
    expect(r.result).toBe(true)
  })

  it('text_filter: prefix mismatch', () => {
    const r = computeNodeParams('text_filter', { input: 'DSC_001.jpg', prefix: 'IMG', suffix: '', contains: '' })
    expect(r.result).toBe(false)
  })

  it('text_filter: contains match (case-insensitive by default)', () => {
    const r = computeNodeParams('text_filter', { input: 'Hello World', prefix: '', suffix: '', contains: 'world', match_case: false })
    expect(r.result).toBe(true)
  })

  it('text_filter: contains mismatch with match_case=true', () => {
    const r = computeNodeParams('text_filter', { input: 'Hello World', prefix: '', suffix: '', contains: 'world', match_case: true })
    expect(r.result).toBe(false)
  })

  // ── Properties ───────────────────────────────────────────────────────────

  const mockMeta: ImageMeta = {
    path: '/images/photo.jpg',
    name: 'photo.jpg',
    sizeBytes: 2048 * 1024,
    width: 1920,
    height: 1080,
    bitDepth: 8,
    extension: 'jpg',
    dpiX: 72,
    dpiY: 72,
    exif: {
      Make: 'Canon',
      Model: 'EOS R5',
      FNumber: '28/10',
      PhotographicSensitivity: '400',
    },
  }

  it('prop_name: returns name', () => {
    const r = computeNodeParams('prop_name', {}, mockMeta)
    expect(r.value).toBe('photo.jpg')
  })

  it('prop_name: strips extension when strip_extension=true', () => {
    const r = computeNodeParams('prop_name', { strip_extension: true }, mockMeta)
    expect(r.value).toBe('photo')
  })

  it('prop_dimensions: returns width and height', () => {
    const r = computeNodeParams('prop_dimensions', {}, mockMeta)
    expect(r.width).toBe(1920)
    expect(r.height).toBe(1080)
  })

  it('prop_size: bytes by default', () => {
    const r = computeNodeParams('prop_size', { unit: 'bytes' }, mockMeta)
    expect(r.value).toBe(2048 * 1024)
  })

  it('prop_size: converts to KB', () => {
    const r = computeNodeParams('prop_size', { unit: 'KB' }, mockMeta)
    expect(r.value).toBe(2048)
  })

  it('prop_size: converts to MB', () => {
    const r = computeNodeParams('prop_size', { unit: 'MB' }, mockMeta)
    expect(r.value).toBe(2)
  })

  it('prop_exif: parses rational FNumber', () => {
    const r = computeNodeParams('prop_exif', {}, mockMeta)
    expect(r.aperture).toBeCloseTo(2.8)
    expect(r.camera_make).toBe('Canon')
    expect(r.camera_model).toBe('EOS R5')
    expect(r.iso).toBe(400)
  })

  // ── value_color ───────────────────────────────────────────────────────────

  it('value_color: splits rgba channels', () => {
    const r = computeNodeParams('value_color', { color: [0.1, 0.2, 0.3, 1.0] })
    expect(r.r).toBeCloseTo(0.1)
    expect(r.g).toBeCloseTo(0.2)
    expect(r.b).toBeCloseTo(0.3)
    expect(r.a).toBeCloseTo(1.0)
    expect(r.rgb).toEqual([0.1, 0.2, 0.3])
    expect(r.rgba).toEqual([0.1, 0.2, 0.3, 1.0])
  })

  // ── unknown executor ──────────────────────────────────────────────────────

  it('unknown executor key: returns params unchanged', () => {
    const params = { x: 1 }
    const r = computeNodeParams('totally_unknown', params)
    expect(r).toEqual(params)
  })

  it('undefined executor key: returns params unchanged', () => {
    const params = { x: 1 }
    const r = computeNodeParams(undefined, params)
    expect(r).toEqual(params)
  })

  // ── prop_power_of_two ──────────────────────────────────────────────────────

  describe('prop_power_of_two', () => {
    const pot = (w: number, h: number) =>
      computeNodeParams('prop_power_of_two', {}, { width: w, height: h } as ImageMeta)

    it('both PoT: all true', () => {
      const r = pot(512, 1024)
      expect(r.width_ok).toBe(true)
      expect(r.height_ok).toBe(true)
      expect(r.result).toBe(true)
    })

    it('width PoT, height not: result false', () => {
      const r = pot(256, 300)
      expect(r.width_ok).toBe(true)
      expect(r.height_ok).toBe(false)
      expect(r.result).toBe(false)
    })

    it('neither PoT', () => {
      const r = pot(100, 200)
      expect(r.width_ok).toBe(false)
      expect(r.height_ok).toBe(false)
      expect(r.result).toBe(false)
    })

    it('1 is a power of two', () => {
      const r = pot(1, 1)
      expect(r.width_ok).toBe(true)
      expect(r.height_ok).toBe(true)
    })

    it('0 is not a power of two', () => {
      expect(pot(0, 1).width_ok).toBe(false)
    })

    it('exact powers: 2, 4, 8, 16, …, 4096', () => {
      for (const n of [2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096]) {
        expect(pot(n, n).result).toBe(true)
      }
    })

    it('non-powers: 3, 6, 100, 1000', () => {
      for (const n of [3, 6, 100, 1000]) {
        expect(pot(n, n).result).toBe(false)
      }
    })
  })
})

// ── buildResizeArgs ───────────────────────────────────────────────────────────

describe('buildResizeArgs', () => {
  it('absolute + preserve: fit-within WxH geometry', () => {
    const args = buildResizeArgs({ mode: 'absolute', width: 800, height: 600, preserve_aspect: true })
    expect(args).toContain('-resize')
    const geom = args[args.indexOf('-resize') + 1]
    expect(geom).toBe('800x600')
  })

  it('absolute + no preserve: forced WxH! geometry', () => {
    const args = buildResizeArgs({ mode: 'absolute', width: 800, height: 600, preserve_aspect: false })
    const geom = args[args.indexOf('-resize') + 1]
    expect(geom).toBe('800x600!')
  })

  it('relative + preserve: uniform percent geometry', () => {
    const args = buildResizeArgs({ mode: 'relative', scale: 50, preserve_aspect: true })
    const geom = args[args.indexOf('-resize') + 1]
    expect(geom).toBe('50%')
  })

  it('relative + no preserve: independent percent geometry', () => {
    const args = buildResizeArgs({ mode: 'relative', scale_width: 75, scale_height: 50, preserve_aspect: false })
    const geom = args[args.indexOf('-resize') + 1]
    expect(geom).toBe('75%x50%!')
  })

  it('defaults: absolute 1024x1024, preserve, Lanczos, 72dpi', () => {
    const args = buildResizeArgs({})
    expect(args).toContain('-resize')
    expect(args[args.indexOf('-resize') + 1]).toBe('1024x1024')
    expect(args).toContain('-filter')
    expect(args[args.indexOf('-filter') + 1]).toBe('Lanczos')
    expect(args).toContain('-density')
    expect(args[args.indexOf('-density') + 1]).toBe('72')
  })

  it('custom filter is passed through', () => {
    const args = buildResizeArgs({ filter: 'Mitchell' })
    expect(args[args.indexOf('-filter') + 1]).toBe('Mitchell')
  })

  it('density arg included when density > 0', () => {
    const args = buildResizeArgs({ density: 300 })
    expect(args).toContain('-density')
    expect(args[args.indexOf('-density') + 1]).toBe('300')
    expect(args).toContain('-units')
    expect(args[args.indexOf('-units') + 1]).toBe('PixelsPerInch')
  })

  it('density arg omitted when density is 0', () => {
    const args = buildResizeArgs({ density: 0, width: 100, height: 100 })
    expect(args).not.toContain('-density')
    expect(args).not.toContain('-units')
  })

  it('clamps width and height to minimum 1', () => {
    const args = buildResizeArgs({ mode: 'absolute', width: 0, height: -5, preserve_aspect: false })
    const geom = args[args.indexOf('-resize') + 1]
    expect(geom).toBe('1x1!')
  })

  it('density arg comes before -resize', () => {
    const args = buildResizeArgs({ density: 150 })
    expect(args.indexOf('-density')).toBeLessThan(args.indexOf('-resize'))
  })
})
