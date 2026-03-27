import { describe, it, expect, vi } from 'vitest'
import { createListenerTracker } from '../../electron/ipcListenerTracker.js'

describe('createListenerTracker', () => {
  it('wrap() returns a different function reference from the original', () => {
    const tracker = createListenerTracker()
    const listener = vi.fn()
    const wrapper = tracker.wrap('ch', listener)
    expect(wrapper).not.toBe(listener)
    expect(typeof wrapper).toBe('function')
  })

  it('wrapper forwards calls to the original listener', () => {
    const tracker = createListenerTracker()
    const listener = vi.fn()
    const wrapper = tracker.wrap('ch', listener)
    wrapper('arg1', 'arg2')
    expect(listener).toHaveBeenCalledWith('arg1', 'arg2')
  })

  it('getWrapper() retrieves the same wrapper that wrap() created', () => {
    const tracker = createListenerTracker()
    const listener = vi.fn()
    const wrapper = tracker.wrap('ch', listener)
    expect(tracker.getWrapper('ch', listener)).toBe(wrapper)
  })

  it('remove() clears the mapping so getWrapper() returns undefined afterwards', () => {
    const tracker = createListenerTracker()
    const listener = vi.fn()
    tracker.wrap('ch', listener)
    tracker.remove('ch', listener)
    expect(tracker.getWrapper('ch', listener)).toBeUndefined()
  })

  it('multiple channels for the same listener are tracked independently', () => {
    const tracker = createListenerTracker()
    const listener = vi.fn()
    const w1 = tracker.wrap('ch1', listener)
    const w2 = tracker.wrap('ch2', listener)
    expect(w1).not.toBe(w2)
    expect(tracker.getWrapper('ch1', listener)).toBe(w1)
    expect(tracker.getWrapper('ch2', listener)).toBe(w2)
    // Removing one channel doesn't affect the other
    tracker.remove('ch1', listener)
    expect(tracker.getWrapper('ch1', listener)).toBeUndefined()
    expect(tracker.getWrapper('ch2', listener)).toBe(w2)
  })

  it('multiple listeners on the same channel are tracked independently', () => {
    const tracker = createListenerTracker()
    const l1 = vi.fn()
    const l2 = vi.fn()
    const w1 = tracker.wrap('ch', l1)
    const w2 = tracker.wrap('ch', l2)
    expect(tracker.getWrapper('ch', l1)).toBe(w1)
    expect(tracker.getWrapper('ch', l2)).toBe(w2)
  })

  it('getWrapper() returns undefined for a listener that was never registered', () => {
    const tracker = createListenerTracker()
    const listener = vi.fn()
    expect(tracker.getWrapper('ch', listener)).toBeUndefined()
  })

  it('remove() on an unregistered listener is a no-op', () => {
    const tracker = createListenerTracker()
    const listener = vi.fn()
    // Should not throw
    expect(() => tracker.remove('ch', listener)).not.toThrow()
  })
})
