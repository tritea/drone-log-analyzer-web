/**
 * wasm parser loader: fetches and initializes the dla-parser module once.
 *
 * `init()` resolves with the RAW wasm exports (ABI-level pointers, not the
 * marshalling wrappers) — the only thing worth keeping from it is `memory`.
 * All calls go through the glue module's own exported functions, which
 * handle string/Uint8Array marshalling.
 */
import init, * as parser from '@/wasm/parser/dla_parser';
// Relative on purpose: a paths-aliased '?url' specifier does not fall back
// to vite/client's ambient `*?url` declaration under vue-tsc.
import wasmUrl from '../../../wasm/parser/dla_parser_bg.wasm?url';

let memory: WebAssembly.Memory | null = null;
let ready: Promise<typeof parser> | null = null;

/** The initialized parser glue module (fetch + compile happen on first call). */
export function parserModule(): Promise<typeof parser> {
  if (!ready) {
    ready = init(wasmUrl).then((exports) => {
      memory = exports.memory;
      return parser;
    });
  }
  return ready;
}

/** The wasm linear memory backing the zero-copy type-body views. */
export function parserMemory(): WebAssembly.Memory {
  if (!memory) throw new Error('wasm parser not initialized');
  return memory;
}
