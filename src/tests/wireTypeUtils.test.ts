import { describe, it, expect } from 'vitest'
import { numericWireTypes, scalarTypes, wireTypesCompatible, paramTypeToWireType, paramInHandle, paramOutHandle } from '../renderer/nodeEditor/wireTypeUtils.js'

describe('numericWireTypes', () => {
  it('contains expected members', () => {
    expect(numericWireTypes.has('number')).toBe(true)
    expect(numericWireTypes.has('numeric')).toBe(true)
    expect(numericWireTypes.has('vector2')).toBe(true)
    expect(numericWireTypes.has('vector3')).toBe(true)
    expect(numericWireTypes.has('vector4')).toBe(true)
    expect(numericWireTypes.has('color')).toBe(true)
  })

  it('does not contain non-numeric types', () => {
    expect(numericWireTypes.has('image')).toBe(false)
    expect(numericWireTypes.has('boolean')).toBe(false)
    expect(numericWireTypes.has('string')).toBe(false)
    expect(numericWireTypes.has('any')).toBe(false)
  })
})

describe('scalarTypes', () => {
  it('contains boolean, number, numeric', () => {
    expect(scalarTypes.has('boolean')).toBe(true)
    expect(scalarTypes.has('number')).toBe(true)
    expect(scalarTypes.has('numeric')).toBe(true)
  })

  it('does not contain vector or image types', () => {
    expect(scalarTypes.has('vector2')).toBe(false)
    expect(scalarTypes.has('color')).toBe(false)
    expect(scalarTypes.has('image')).toBe(false)
  })
})

describe('wireTypesCompatible', () => {
  describe('identity', () => {
    it('same type is always compatible', () => {
      for (const t of ['image', 'mask', 'number', 'boolean', 'string', 'color', 'vector3', 'any', 'path']) {
        expect(wireTypesCompatible(t, t)).toBe(true)
      }
    })
  })

  describe('any wildcard', () => {
    it('any matches everything', () => {
      expect(wireTypesCompatible('any', 'image')).toBe(true)
      expect(wireTypesCompatible('image', 'any')).toBe(true)
      expect(wireTypesCompatible('any', 'boolean')).toBe(true)
      expect(wireTypesCompatible('any', 'vector4')).toBe(true)
      expect(wireTypesCompatible('number', 'any')).toBe(true)
    })
  })

  describe('numeric coercion via "numeric"', () => {
    it('numeric is compatible with number', () => {
      expect(wireTypesCompatible('numeric', 'number')).toBe(true)
      expect(wireTypesCompatible('number', 'numeric')).toBe(true)
    })

    it('numeric is compatible with vector types', () => {
      expect(wireTypesCompatible('numeric', 'vector2')).toBe(true)
      expect(wireTypesCompatible('numeric', 'vector3')).toBe(true)
      expect(wireTypesCompatible('numeric', 'vector4')).toBe(true)
      expect(wireTypesCompatible('numeric', 'color')).toBe(true)
    })

    it('numeric does NOT bridge to non-numeric non-scalar types', () => {
      expect(wireTypesCompatible('numeric', 'image')).toBe(false)
      expect(wireTypesCompatible('numeric', 'string')).toBe(false)
    })

    it('numeric IS compatible with boolean via scalarTypes', () => {
      // boolean, number, numeric are all in scalarTypes → mutually compatible
      expect(wireTypesCompatible('numeric', 'boolean')).toBe(true)
    })

    it('non-numeric types are not compatible with each other via numeric', () => {
      expect(wireTypesCompatible('vector2', 'vector3')).toBe(false)
      expect(wireTypesCompatible('color', 'vector3')).toBe(false)
    })
  })

  describe('scalar inter-compatibility (boolean / number / numeric)', () => {
    it('boolean is compatible with number', () => {
      expect(wireTypesCompatible('boolean', 'number')).toBe(true)
      expect(wireTypesCompatible('number', 'boolean')).toBe(true)
    })

    it('boolean is compatible with numeric', () => {
      expect(wireTypesCompatible('boolean', 'numeric')).toBe(true)
      expect(wireTypesCompatible('numeric', 'boolean')).toBe(true)
    })
  })

  describe('incompatible pairs', () => {
    it('image and mask are not cross-compatible', () => {
      expect(wireTypesCompatible('image', 'mask')).toBe(false)
    })

    it('image is not compatible with number', () => {
      expect(wireTypesCompatible('image', 'number')).toBe(false)
    })

    it('string is not compatible with number', () => {
      expect(wireTypesCompatible('string', 'number')).toBe(false)
    })

    it('path is not compatible with image', () => {
      expect(wireTypesCompatible('path', 'image')).toBe(false)
    })
  })
})

// ── paramTypeToWireType ────────────────────────────────────────────────────────

describe('paramTypeToWireType', () => {
  it('maps bool → boolean', () => expect(paramTypeToWireType('bool')).toBe('boolean'))
  it('maps string → string', () => expect(paramTypeToWireType('string')).toBe('string'))
  it('maps vector2 → vector2', () => expect(paramTypeToWireType('vector2')).toBe('vector2'))
  it('maps vector3 → vector3', () => expect(paramTypeToWireType('vector3')).toBe('vector3'))
  it('maps vector4 → vector4', () => expect(paramTypeToWireType('vector4')).toBe('vector4'))
  it('maps color → color', () => expect(paramTypeToWireType('color')).toBe('color'))
  it('maps numeric → numeric', () => expect(paramTypeToWireType('numeric')).toBe('numeric'))
  it('maps any → any', () => expect(paramTypeToWireType('any')).toBe('any'))
  it('maps int → number', () => expect(paramTypeToWireType('int')).toBe('number'))
  it('maps float → number', () => expect(paramTypeToWireType('float')).toBe('number'))
  it('maps unknown type → number', () => expect(paramTypeToWireType('unknown_type')).toBe('number'))
})

// ── paramInHandle / paramOutHandle ────────────────────────────────────────────

describe('paramInHandle', () => {
  it('produces param-in-<name>', () => {
    expect(paramInHandle('width')).toBe('param-in-width')
    expect(paramInHandle('_enabled')).toBe('param-in-_enabled')
    expect(paramInHandle('scale_width')).toBe('param-in-scale_width')
  })
})

describe('paramOutHandle', () => {
  it('produces param-out-<name>', () => {
    expect(paramOutHandle('result')).toBe('param-out-result')
    expect(paramOutHandle('width_ok')).toBe('param-out-width_ok')
  })
})
