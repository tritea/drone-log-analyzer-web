/**
 * wasm-backed LogClient: parses in the frontend via the Rust wasm module.
 *
 * Data flow:
 * - `load` fetches the raw log over `/rawlog` (Go serves the picked file from
 *   a whitelist) and feeds it to wasm in rAF-spaced chunks so the parse
 *   progress UI can update; `parseFinish` returns every logservice projection
 *   in one JSON payload, cached here.
 * - `typeBody` returns a zero-copy Uint8Array VIEW into wasm linear memory
 *   (never a copy). The contract: wasm allocates nothing after parseFinish,
 *   so the view stays valid until the next load's parseStart.
 */
import type { LogClient } from '../client';
import type { logservice } from '../types';
import { parserMemory, parserModule } from './loader';

/** Everything except curve bodies, frozen at finish. */
interface ParsePayload {
  summary: logservice.SummaryResponse;
  messageTypes: logservice.TypeInfo[];
  fields: Record<string, logservice.FieldInfo[]>;
  typeSchema: Record<string, unknown>;
  parameters: logservice.Parameter[];
  commands: logservice.CommandEntry[];
  mavlinkCommands: logservice.MAVLinkCommandEntry[];
  modeChanges: logservice.ModeEntry[];
  messages: logservice.MessageEntry[];
  errors: logservice.ErrorEntry[];
  events: logservice.EventEntry[];
  logDefs: logservice.LogDefsResponse;
}

const FEED_CHUNK_BYTES = 1 << 20; // 1 MiB per wasm feed call
/** Yield to the event loop every N chunks so the parse progress can paint. */
const CHUNKS_PER_FRAME = 8;

function nextFrame(): Promise<void> {
  return new Promise((resolve) => {
    if (typeof requestAnimationFrame === 'function') requestAnimationFrame(() => resolve());
    else setTimeout(resolve, 0);
  });
}

let payload: ParsePayload | null = null;
let loadedPath = '';

function requirePayload(): ParsePayload {
  if (!payload) throw new Error('no log loaded');
  return payload;
}

export const wasmLogClient: LogClient = {
  async load(file: File) {
    const bytes = new Uint8Array(await file.arrayBuffer());

    const mod = await parserModule();
    mod.parseStart(file.name);
    let fed = 0;
    let chunksInFrame = 0;
    while (fed < bytes.length) {
      const end = Math.min(fed + FEED_CHUNK_BYTES, bytes.length);
      // The subarray copy into wasm memory is the pipeline's single memcpy.
      mod.parseFeed(bytes.subarray(fed, end));
      fed = end;
      if (++chunksInFrame >= CHUNKS_PER_FRAME && fed < bytes.length) {
        chunksInFrame = 0;
        await nextFrame();
      }
    }
    const parsed = JSON.parse(mod.parseFinish()) as ParsePayload;
    if (!parsed || !parsed.summary) throw new Error('parse failed');
    payload = parsed;
    loadedPath = file.name;
    return parsed.summary;
  },

  async status(): Promise<logservice.StatusResponse> {
    const mod = await parserModule();
    return { loaded: mod.isLoaded(), fileName: loadedPath };
  },

  async summary() {
    return requirePayload().summary;
  },

  async messageTypes() {
    return requirePayload().messageTypes;
  },

  async fields(req) {
    const fields = requirePayload().fields[req.type];
    if (!fields) throw new Error(`type not found: ${req.type}`);
    return fields;
  },

  async typeSchema() {
    return requirePayload().typeSchema;
  },

  async typeBody(req): Promise<ArrayBuffer | Uint8Array> {
    const mod = await parserModule();
    const ptr = mod.typeBodyPtr(req.type);
    if (!ptr) throw new Error(`type not found: ${req.type}`);
    const len = mod.typeBodyLen(req.type);
    // Zero-copy view straight into wasm linear memory; parseTypeBody accepts
    // it and builds a positioned DataView. Valid until the next parseStart.
    return new Uint8Array(parserMemory().buffer, ptr, len);
  },

  async parameters() {
    return requirePayload().parameters;
  },

  async commands() {
    return requirePayload().commands;
  },

  async mavlinkCommands() {
    return requirePayload().mavlinkCommands;
  },

  async modeChanges() {
    return requirePayload().modeChanges;
  },

  async messages() {
    return requirePayload().messages;
  },

  async errors() {
    return requirePayload().errors;
  },

  async events() {
    return requirePayload().events;
  },

  async logDefs() {
    return requirePayload().logDefs;
  },
};
