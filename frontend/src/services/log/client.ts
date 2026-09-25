import type { logservice } from './types';

export interface LogClient {
  /** Parses a picked log file in wasm (browser File object, no server round-trip). */
  load(file: File): Promise<logservice.SummaryResponse>;
  status(): Promise<logservice.StatusResponse>;
  summary(): Promise<logservice.SummaryResponse>;
  messageTypes(): Promise<logservice.TypeInfo[]>;
  fields(req: logservice.FieldsRequest): Promise<logservice.FieldInfo[]>;
  typeSchema(): Promise<Record<string, any>>;
  /**
   * The packed NUTB body of a type. The wasm client returns a zero-copy
   * Uint8Array view into wasm linear memory (callers must not retain it
   * across loads).
   */
  typeBody(req: logservice.TypeBodyRequest): Promise<ArrayBuffer | Uint8Array>;
  parameters(): Promise<logservice.Parameter[]>;
  commands(): Promise<logservice.CommandEntry[]>;
  mavlinkCommands(): Promise<logservice.MAVLinkCommandEntry[]>;
  modeChanges(): Promise<logservice.ModeEntry[]>;
  messages(): Promise<logservice.MessageEntry[]>;
  errors(): Promise<logservice.ErrorEntry[]>;
  events(): Promise<logservice.EventEntry[]>;
  logDefs(): Promise<logservice.LogDefsResponse>;
}
