#!/usr/bin/env node
/**
 * Builds the Rust wasm parser into frontend/src/wasm/parser (wasm-pack,
 * --target web). Skips when the package is already newer than the Rust
 * sources, so repeated vite builds stay fast; force with --force.
 *
 * cargo/wasm-pack live in ~/.cargo/bin which is often not on PATH.
 */
import { execFileSync } from 'node:child_process'
import { existsSync, statSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const crateDir = resolve(root, 'wasm-parser')
const outDir = resolve(root, 'frontend/src/wasm/parser')
const marker = resolve(outDir, 'dla_parser_bg.wasm')
const force = process.argv.includes('--force')

function findTool(name) {
  const pathDirs = (process.env.PATH || '').split(';')
  const candidates = [
    resolve(process.env.USERPROFILE || '', '.cargo/bin', `${name}.exe`),
    resolve(process.env.USERPROFILE || '', '.cargo/bin', name),
    ...pathDirs.map((d) => resolve(d, `${name}.exe`)),
    ...pathDirs.map((d) => resolve(d, name)),
  ]
  return candidates.find((p) => existsSync(p))
}

function rustNewer() {
  if (!existsSync(marker)) return true
  const built = statSync(marker).mtimeMs
  const walk = (dir, out) => {
    for (const e of execFileSync('cmd', ['/c', 'dir', '/s', '/b', dir])
      .toString()
      .split(/\r?\n/)
      .filter(Boolean)) {
      if (/\.(rs|toml|json)$/.test(e) && statSync(e).mtimeMs > built) return false
    }
    return true
  }
  return walk(resolve(crateDir, 'src')) && walk(resolve(crateDir, 'data'))
}

if (!force && existsSync(marker) && rustNewer()) {
  console.log('[build-wasm] frontend/src/wasm/parser is up to date')
  process.exit(0)
}

const wasmPack = findTool('wasm-pack')
if (!wasmPack) {
  console.error('[build-wasm] wasm-pack not found (looked in PATH and ~/.cargo/bin)')
  process.exit(1)
}

console.log('[build-wasm] running wasm-pack build')
execFileSync(wasmPack, [
  'build', '--release', '--target', 'web', '--out-dir', outDir,
], { stdio: 'inherit', cwd: crateDir })
console.log('[build-wasm] done')
