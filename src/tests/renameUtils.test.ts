import { describe, it, expect } from 'vitest'
import { computeNewName, previewRenames } from '../shared/renameUtils.js'
import type { RenameParams } from '../shared/renameUtils.js'

// ── computeNewName ────────────────────────────────────────────────────────────

describe('computeNewName', () => {
  it('returns original name when blocks is empty', () => {
    expect(computeNewName('photo.jpg', {}, 0)).toBe('photo.jpg')
  })

  it('returns original name when blocks is undefined', () => {
    const params: RenameParams = {}
    expect(computeNewName('photo.jpg', params, 0)).toBe('photo.jpg')
  })

  describe('text block', () => {
    it('outputs literal text with extension', () => {
      const params: RenameParams = { blocks: [{ type: 'text', value: 'hero' }] }
      expect(computeNewName('photo.jpg', params, 0)).toBe('hero.jpg')
    })

    it('preserves extension from original', () => {
      const params: RenameParams = { blocks: [{ type: 'text', value: 'shot' }] }
      expect(computeNewName('img.PNG', params, 0)).toBe('shot.PNG')
    })

    it('concatenates multiple text blocks', () => {
      const params: RenameParams = {
        blocks: [
          { type: 'text', value: 'hero' },
          { type: 'text', value: '_wide' },
        ],
      }
      expect(computeNewName('photo.jpg', params, 0)).toBe('hero_wide.jpg')
    })
  })

  describe('number block', () => {
    it('produces zero-padded sequence starting at start value', () => {
      const params: RenameParams = { blocks: [{ type: 'number', start: 1, pad: 3 }] }
      expect(computeNewName('a.jpg', params, 0)).toBe('001.jpg')
      expect(computeNewName('b.jpg', params, 1)).toBe('002.jpg')
      expect(computeNewName('c.jpg', params, 9)).toBe('010.jpg')
    })

    it('respects custom start value', () => {
      const params: RenameParams = { blocks: [{ type: 'number', start: 10, pad: 2 }] }
      expect(computeNewName('a.jpg', params, 0)).toBe('10.jpg')
      expect(computeNewName('a.jpg', params, 5)).toBe('15.jpg')
    })

    it('respects custom pad', () => {
      const params: RenameParams = { blocks: [{ type: 'number', start: 1, pad: 5 }] }
      expect(computeNewName('a.jpg', params, 0)).toBe('00001.jpg')
    })

    it('clamps pad to minimum 1', () => {
      const params: RenameParams = { blocks: [{ type: 'number', start: 1, pad: 0 }] }
      // pad = Math.max(1, 0) = 1
      expect(computeNewName('a.jpg', params, 0)).toBe('1.jpg')
    })

    it('clamps start to minimum 0', () => {
      const params: RenameParams = { blocks: [{ type: 'number', start: -5, pad: 2 }] }
      expect(computeNewName('a.jpg', params, 0)).toBe('00.jpg')
    })
  })

  describe('oldname block', () => {
    it('uses original stem when find is empty', () => {
      const params: RenameParams = { blocks: [{ type: 'oldname', find: '', replace_with: '' }] }
      expect(computeNewName('photo_001.jpg', params, 0)).toBe('photo_001.jpg')
    })

    it('replaces all occurrences of find in stem', () => {
      const params: RenameParams = { blocks: [{ type: 'oldname', find: '_', replace_with: '-' }] }
      expect(computeNewName('my_photo_001.jpg', params, 0)).toBe('my-photo-001.jpg')
    })

    it('removes find when replace_with is empty', () => {
      const params: RenameParams = { blocks: [{ type: 'oldname', find: 'IMG_', replace_with: '' }] }
      expect(computeNewName('IMG_1234.jpg', params, 0)).toBe('1234.jpg')
    })
  })

  describe('mixed blocks', () => {
    it('concatenates text + oldname + number', () => {
      const params: RenameParams = {
        blocks: [
          { type: 'text', value: 'hero_' },
          { type: 'oldname', find: '', replace_with: '' },
          { type: 'text', value: '_' },
          { type: 'number', start: 1, pad: 2 },
        ],
      }
      expect(computeNewName('shot.jpg', params, 0)).toBe('hero_shot_01.jpg')
      expect(computeNewName('shot.jpg', params, 4)).toBe('hero_shot_05.jpg')
    })
  })

  describe('extension handling', () => {
    it('file without extension', () => {
      const params: RenameParams = { blocks: [{ type: 'text', value: 'renamed' }] }
      expect(computeNewName('noext', params, 0)).toBe('renamed')
    })

    it('leading dot files treated as no-extension', () => {
      // dot at index 0 → no extension split
      const params: RenameParams = { blocks: [{ type: 'text', value: 'out' }] }
      expect(computeNewName('.gitignore', params, 0)).toBe('out')
    })
  })
})

// ── previewRenames ────────────────────────────────────────────────────────────

describe('previewRenames', () => {
  it('returns one entry per file', () => {
    const files = ['a.jpg', 'b.jpg', 'c.jpg']
    const result = previewRenames(files, {})
    expect(result).toHaveLength(3)
  })

  it('marks unchanged entries correctly', () => {
    const files = ['a.jpg', 'b.jpg']
    // Empty params → original name returned → not changed
    const result = previewRenames(files, {})
    expect(result[0].changed).toBe(false)
    expect(result[0].original).toBe('a.jpg')
    expect(result[0].newName).toBe('a.jpg')
  })

  it('marks changed entries correctly', () => {
    const params: RenameParams = { blocks: [{ type: 'text', value: 'new' }] }
    const result = previewRenames(['old.jpg'], params)
    expect(result[0].changed).toBe(true)
    expect(result[0].original).toBe('old.jpg')
    expect(result[0].newName).toBe('new.jpg')
  })

  it('increments index per file for number blocks', () => {
    const params: RenameParams = { blocks: [{ type: 'number', start: 1, pad: 2 }] }
    const result = previewRenames(['a.jpg', 'b.jpg', 'c.jpg'], params)
    expect(result[0].newName).toBe('01.jpg')
    expect(result[1].newName).toBe('02.jpg')
    expect(result[2].newName).toBe('03.jpg')
  })
})
