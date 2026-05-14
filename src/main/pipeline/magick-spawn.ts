import { spawn } from 'node:child_process'
import { getMagickBinary } from './magick-path.js'

export function spawnMagick(args: string[], bucket?: { add(ms: number): void }): Promise<void> {
  const t0 = bucket ? Date.now() : 0
  return new Promise((resolve, reject) => {
    const proc = spawn(getMagickBinary(), args)
    const stderr: string[] = []
    proc.stderr.on('data', (chunk: Buffer) => stderr.push(chunk.toString()))
    proc.on('close', (code) => {
      if (bucket) bucket.add(Date.now() - t0)
      if (code === 0) resolve()
      else reject(new Error(`magick exited ${code}: ${stderr.join('').trim()}`))
    })
    proc.on('error', (err) => {
      if (bucket) bucket.add(Date.now() - t0)
      reject(new Error(`Failed to spawn magick: ${err.message}. Is ImageMagick v7 in PATH?`))
    })
  })
}
