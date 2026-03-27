import { describe, it, expect, vi, beforeEach } from 'vitest'

// Set up window.ipcRenderer mock before importing the service
const mockOn     = vi.fn()
const mockOff    = vi.fn()
const mockInvoke = vi.fn()

;(globalThis as Record<string, unknown>).window = {
  ipcRenderer: { on: mockOn, off: mockOff, invoke: mockInvoke },
}

// Dynamic import so the mock is in place when the module initialises
const { ElectronPipelineService } = await import('../renderer/services/pipeline-service.js')

const BATCH_PROGRESS_CHANNEL = 'pipeline:execute-batch:progress'

describe('ElectronPipelineService.executeBatch', () => {
  beforeEach(() => {
    mockOn.mockReset()
    mockOff.mockReset()
    mockInvoke.mockReset()
  })

  it('registers a progress listener and removes it after a successful invoke', async () => {
    mockInvoke.mockResolvedValueOnce(undefined)
    const svc = new ElectronPipelineService()
    await svc.executeBatch({} as never, [], '', vi.fn())

    expect(mockOn).toHaveBeenCalledOnce()
    expect(mockOn).toHaveBeenCalledWith(BATCH_PROGRESS_CHANNEL, expect.any(Function))
    expect(mockOff).toHaveBeenCalledOnce()

    // The exact same listener reference must be used for on and off
    const registeredListener = mockOn.mock.calls[0][1]
    const removedListener    = mockOff.mock.calls[0][1]
    expect(registeredListener).toBe(removedListener)
  })

  it('removes the progress listener even when invoke rejects (finally block runs)', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('batch failed'))
    const svc = new ElectronPipelineService()
    await expect(svc.executeBatch({} as never, [], '', vi.fn())).rejects.toThrow('batch failed')

    expect(mockOff).toHaveBeenCalledOnce()
  })

  it('each call registers and removes its own independent listener (no stacking)', async () => {
    // Two sequential calls — each should have exactly one on/off pair
    mockInvoke.mockResolvedValue(undefined)
    const svc = new ElectronPipelineService()
    await svc.executeBatch({} as never, [], '', vi.fn())
    await svc.executeBatch({} as never, [], '', vi.fn())

    expect(mockOn).toHaveBeenCalledTimes(2)
    expect(mockOff).toHaveBeenCalledTimes(2)

    // Each call must have used a distinct listener instance
    const listener1 = mockOn.mock.calls[0][1]
    const listener2 = mockOn.mock.calls[1][1]
    expect(listener1).not.toBe(listener2)
  })
})
