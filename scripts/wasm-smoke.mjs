#!/usr/bin/env node
/**
 * Runtime smoke test for the wasm parser package: loads the built module in
 * node (initSync), feeds each golden fixture through the full
 * parseStart/parseFeed/parseFinish lifecycle and checks the payload plus a
 * type-body pointer. Usage: node scripts/wasm-smoke.mjs [fixture.tlog ...]
 */
import { readFileSync } from 'node:fs'
import { basename, resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  initSync,
  isLoaded,
  parseFeed,
  parseFinish,
  parseStart,
  signalQuery,
  typeBodyLen,
  typeBodyPtr,
} from '../frontend/src/wasm/parser/dla_parser.js'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
initSync(readFileSync(resolve(root, 'frontend/src/wasm/parser/dla_parser_bg.wasm')))

const fixtures = process.argv.slice(2)
if (fixtures.length === 0) {
  console.error('usage: node scripts/wasm-smoke.mjs <log files...>')
  process.exit(1)
}

let failed = false
for (const file of fixtures) {
  const name = basename(file)
  try {
    const bytes = readFileSync(file)
    parseStart(name)
    for (let off = 0; off < bytes.length; off += 1 << 20) {
      parseFeed(bytes.subarray(off, Math.min(off + (1 << 20), bytes.length)))
    }
    const payload = JSON.parse(parseFinish())
    if (!isLoaded()) throw new Error('isLoaded false after finish')
    const s = payload.summary ?? {}
    const types = payload.messageTypes?.length ?? 0
    const firstName = payload.messageTypes?.[0]?.name ?? ''
    const ptr = typeBodyPtr(firstName)
    const len = typeBodyLen(firstName)
    if (!ptr || !len) throw new Error(`type body missing for "${firstName}" (ptr=${ptr} len=${len})`)
    // Signal query lifecycle: one stats-family op through the wasm boundary
    // (the linear-memory view guard lives in the frontend loader, which is
    // the only place holding the memory object).
    const firstField = payload.messageTypes?.[0]?.fields?.find(
      (f) => typeof f === 'string' && !/time/i.test(f),
    )
    const sig = JSON.parse(
      signalQuery(JSON.stringify({ type: firstName, field: firstField ?? 'TimeMS', op: 'minmax' })),
    )
    if (sig.error) throw new Error(`signalQuery: ${sig.error}`)
    const statsOk = sig.stats && sig.stats.ok === true
    if (!statsOk && sig.n !== 0) throw new Error(`signalQuery stats missing: ${JSON.stringify(sig)}`)
    const ptrAfter = typeBodyPtr(firstName)
    if (ptrAfter !== ptr) throw new Error('signalQuery moved the type body (view invalidation!)')
    console.log(
      `[smoke] ${name}: format=${s.format} types=${types} params=${payload.parameters?.length ?? 0}` +
        ` msgs=${payload.messages?.length ?? 0} duration=${s.durationSecs}s` +
        ` body0=${payload.messageTypes?.[0]?.name}(${len}B)` +
        ` signal=${firstName}.${firstField ?? 'TimeMS'}:n=${sig.n} OK`,
    )
  } catch (err) {
    failed = true
    console.error(`[smoke] ${name}: FAILED — ${err?.message ?? err}`)
  }
}
process.exit(failed ? 1 : 0)
