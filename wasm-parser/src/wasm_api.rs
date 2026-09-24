//! wasm32 bindings: the frontend parse lifecycle plus query accessors.
//!
//! Zero-copy contract, by construction:
//! - `parse_finish` serializes the whole JSON payload (summary + every
//!   logservice projection) in ONE shot, BEFORE any pointer is handed out.
//! - Afterwards the only wasm calls are `type_body_ptr`/`type_body_len`,
//!   which allocate nothing — so (ptr, len) views into wasm linear memory
//!   stay valid until the next `parse_start` drops the LogFile.
//! - During feeding, wasm memory may grow freely; the frontend holds no
//!   views until finish returns.

use std::cell::RefCell;

use wasm_bindgen::prelude::*;

use crate::logfile::LogFile;
use crate::query;
use crate::Session;

enum State {
    Idle,
    Parsing(Session),
    Done(Finished),
}

struct Finished {
    file_name: String,
    log: LogFile,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::Idle);
}

fn with_log<T>(f: impl FnOnce(&LogFile) -> T) -> Result<T, JsError> {
    STATE.with(|s| {
        let st = s.borrow();
        match &*st {
            State::Done(fin) => Ok(f(&fin.log)),
            _ => Err(JsError::new("no log loaded")),
        }
    })
}

/// Starts a parse. Drops any previous generation's LogFile wholesale.
#[wasm_bindgen(js_name = parseStart)]
pub fn parse_start(filename: String) -> u32 {
    let generation = crate::next_generation();
    STATE.with(|s| {
        *s.borrow_mut() = State::Parsing(Session::new(generation, &filename));
    });
    generation
}

/// Feeds one chunk of raw file bytes. Returns the total bytes fed so far
/// (drives the progress UI).
#[wasm_bindgen(js_name = parseFeed)]
pub fn parse_feed(chunk: &[u8]) -> Result<u64, JsError> {
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        match &mut *st {
            State::Parsing(session) => session
                .feed(chunk)
                .map_err(|e| JsError::new(&e.to_string())),
            _ => Err(JsError::new("no parse in progress")),
        }
    })
}

/// Finishes the parse and returns the full query payload as ONE JSON string:
/// { summary, messageTypes, fields, typeSchema, parameters, commands,
///   mavlinkCommands, modeChanges, messages, errors, events, logDefs }.
/// A string (parsed with JSON.parse on the JS side) rather than a
/// structured value: serde_json::Value objects would cross the boundary as
/// JS Maps, and both sides use shortest-round-trip decimals, so the f64
/// values survive exactly. Everything except curve bodies lives on the JS
/// side after this call.
#[wasm_bindgen(js_name = parseFinish)]
pub fn parse_finish() -> Result<String, JsError> {
    let finished = STATE.with(|s| {
        let mut st = s.borrow_mut();
        match std::mem::replace(&mut *st, State::Idle) {
            State::Parsing(mut session) => {
                session.finish().map_err(|e| JsError::new(&e.to_string()))?;
                let file_name = session.filename().to_string();
                Ok(Finished {
                    file_name,
                    log: session.into_log(),
                })
            }
            other => {
                *st = other;
                Err(JsError::new("no parse in progress"))
            }
        }
    })?;

    // Serialize everything BEFORE publishing the finished state, so every
    // allocation happens before the first type-body pointer is requested.
    let payload = build_payload(&finished.log);
    let json = serde_json::to_string(&payload).map_err(|e| JsError::new(&e.to_string()))?;

    STATE.with(|s| {
        *s.borrow_mut() = State::Done(finished);
    });
    Ok(json)
}

fn build_payload(log: &LogFile) -> serde_json::Value {
    let fields: serde_json::map::Map<String, serde_json::Value> = query::message_types(log)
        .into_iter()
        .map(|t| {
            let name = t.name.clone();
            let fs = query::fields(log, &name).unwrap_or_default();
            (name, serde_json::to_value(fs).unwrap_or_default())
        })
        .collect();
    serde_json::json!({
        "summary": serde_json::to_value(query::summary(log)).unwrap_or_default(),
        "messageTypes": serde_json::to_value(query::message_types(log)).unwrap_or_default(),
        "fields": fields,
        "typeSchema": serde_json::to_value(log.type_schema()).unwrap_or_default(),
        "parameters": query::parameters(log),
        "commands": serde_json::to_value(query::commands(log)).unwrap_or_default(),
        "mavlinkCommands": serde_json::to_value(query::mavlink_commands(log)).unwrap_or_default(),
        "modeChanges": serde_json::to_value(query::mode_changes(log)).unwrap_or_default(),
        "messages": serde_json::to_value(query::messages(log)).unwrap_or_default(),
        "errors": serde_json::to_value(query::errors(log)).unwrap_or_default(),
        "events": serde_json::to_value(query::events(log)).unwrap_or_default(),
        "logDefs": serde_json::to_value(query::log_defs(log)).unwrap_or_default(),
    })
}

/// The loaded log's display filename (status panel).
#[wasm_bindgen(js_name = fileName)]
pub fn file_name() -> String {
    STATE.with(|s| {
        let st = s.borrow();
        match &*st {
            State::Done(fin) => fin.file_name.clone(),
            State::Parsing(session) => session.filename().to_string(),
            State::Idle => String::new(),
        }
    })
}

/// Whether a finished log is available for queries.
#[wasm_bindgen(js_name = isLoaded)]
pub fn is_loaded() -> bool {
    STATE.with(|s| matches!(&*s.borrow(), State::Done(_)))
}

/// Pointer of a TypeBody blob inside wasm linear memory (0 when the type has
/// no body). Allocates nothing — valid until the next parseStart.
#[wasm_bindgen(js_name = typeBodyPtr)]
pub fn type_body_ptr(type_name: String) -> u32 {
    with_log(|log| {
        log.type_body_bytes(&type_name)
            .map(|b| b.as_ptr() as u32)
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

/// Byte length of the same TypeBody blob.
#[wasm_bindgen(js_name = typeBodyLen)]
pub fn type_body_len(type_name: String) -> u32 {
    with_log(|log| {
        log.type_body_bytes(&type_name)
            .map(|b| b.len() as u32)
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

/// Runs one agent signal query (`{type, field, startSec, endSec, op,
/// threshold, conds, maxPoints, mergeGapSecs}`) against the loaded log and
/// returns the computed payload as a JSON string (see `signal::run_query`).
///
/// Unlike the type-body accessors this DOES allocate after finish — the
/// response strings are KB-scale and served from the allocator's free
/// regions (the multi-MB input buffer is freed at finish), so the live
/// TypeBody blobs never move and their (ptr, len) views stay valid. The
/// frontend nevertheless re-checks the memory buffer identity after each
/// call as a belt-and-braces view guard.
#[wasm_bindgen(js_name = signalQuery)]
pub fn signal_query(req: String) -> Result<String, JsError> {
    with_log(|log| crate::signal::run_query_json(log, &req))
        .and_then(|inner| inner.map_err(|e: String| JsError::new(&e)))
}
