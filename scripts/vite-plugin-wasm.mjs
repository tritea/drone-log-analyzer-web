/**
 * Vite plugin: ensures the Rust wasm parser package exists before the
 * frontend builds (wails build / vite build / vite dev all pass through
 * here). The heavy wasm-pack step is skipped when up to date — see
 * scripts/build-wasm.mjs.
 */
import { execFileSync } from 'node:child_process'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')

export function wasmParser() {
  return {
    name: 'dla-wasm-parser',
    buildStart() {
      execFileSync(process.execPath, [resolve(root, 'scripts/build-wasm.mjs')], {
        stdio: 'inherit',
      })
    },
  }
}
