import { spawn } from 'node:child_process'
import { getMagickBinary } from './magick-path.js'

export function spawnMagick(
  args: string[],
  bucket?: { add(ms: number): void },
  timeoutMs?: number
): Promise<void> {
  const t0 = bucket ? Date.now() : 0
  return new Promise((resolve, reject) => {
    const proc = spawn(getMagickBinary(), args)
    const stderr: string[] = []

    let timer: ReturnType<typeof setTimeout> | undefined
    if (timeoutMs !== undefined) {
      timer = setTimeout(() => {
        proc.kill()
        if (bucket) bucket.add(Date.now() - t0)
        reject(new Error(`magick timed out after ${timeoutMs}ms`))
      }, timeoutMs)
    }

    proc.stderr.on('data', (chunk: Buffer) => stderr.push(chunk.toString()))
    proc.on('close', (code) => {
      clearTimeout(timer)
      if (bucket) bucket.add(Date.now() - t0)
      if (code === 0) resolve()
      else reject(new Error(`magick exited ${code}: ${stderr.join('').trim()}`))
    })
    proc.on('error', (err) => {
      clearTimeout(timer)
      if (bucket) bucket.add(Date.now() - t0)
      reject(new Error(`Failed to spawn magick: ${err.message}. Is ImageMagick v7 in PATH?`))
    })
  })
}
