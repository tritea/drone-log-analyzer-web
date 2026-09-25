/**
 * Log-service DTO shapes, handwritten to mirror the JSON the query layer
 * emits (previously the wailsjs generated models). Field names must stay in
 * lockstep with `wasm-parser/src/query.rs` (which froze them against the Go
 * goldens); non-finite floats arrive as the string sentinels
 * "NaN" / "Inf" / "-Inf".
 */
export namespace logservice {
  export interface LoadRequest {
    path: string;
  }

  export interface StatusResponse {
    loaded: boolean;
    fileName: string;
  }

  export interface SummaryResponse {
    filename: string;
    fileSizeKB: number;
    vehicleType: string;
    firmwareVersion: string;
    firmwareHash: string;
    hardwareType: string;
    freeRAM: number;
    durationSecs: number;
    totalLines: number;
    frame: string;
    airframe: string;
    typeCount: number;
    startUnixSecs: number;
    hasUTC: boolean;
    /** Absolute-ms origin of the log's relative-second zero point. */
    startTimeMs: number;
    format: string;
  }

  export interface TypeInfo {
    name: string;
    fields: string[];
    count: number;
    hasData: boolean;
  }

  export interface FieldsRequest {
    type: string;
  }

  export interface FieldInfo {
    name: string;
    type: string;
    min: number;
    max: number;
    count: number;
    isNumeric: boolean;
  }

  export interface TypeBodyRequest {
    type: string;
  }

  export interface Parameter {
    name: string;
    value: number | string;
  }

  export interface CommandEntry {
    timeMs: number;
    commandTotal: number;
    sequence: number;
    command: number;
    commandName: string;
    param1: number | string;
    param2: number | string;
    param3: number | string;
    param4: number | string;
    latitude: number | string;
    longitude: number | string;
    altitude: number | string;
    frame: number;
    frameName: string;
  }

  export interface MAVLinkCommandEntry {
    timeMs: number;
    targetSystem: number;
    targetComponent: number;
    sourceSystem: number;
    sourceComponent: number;
    frame: number;
    frameName: string;
    command: number;
    commandName: string;
    param1: number | string;
    param2: number | string;
    param3: number | string;
    param4: number | string;
    latitude: number | string;
    longitude: number | string;
    altitude: number | string;
    result: number;
    resultName: string;
    wasCommandLong: boolean;
  }

  export interface ModeEntry {
    lineno: number;
    timeMs: number;
    mode: string;
    modeNum: number;
  }

  export interface MessageEntry {
    lineno: number;
    timeMs: number;
    message: string;
  }

  export interface ErrorEntry {
    lineno: number;
    timeMs: number;
    subsys: number;
    eCode: number;
    subsysName: string;
    errorCode: string;
    description: string;
  }

  export interface EventEntry {
    lineno: number;
    timeMs: number;
    id: number;
    name: string;
  }

  export interface LogDefsResponse {
    format: string;
    units: Record<string, string>;
    eventNames?: Record<string, string>;
    errorSubsystems?: Record<string, string>;
    errorCodes?: Record<string, Record<string, string>>;
    generalErrorCodes?: Record<string, string>;
    navStateNames?: Record<string, string>;
  }
}
