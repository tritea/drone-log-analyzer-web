//! dla-parser: flight-log parsing core (dataflash / ulog / tlog) for the
//! drone-log-analyzer frontend. Compiles to wasm32 for the app; format
//! backends are incremental state machines driven chunk-by-chunk so the main
//! thread never blocks on one giant parse call.

pub mod accumulate;
pub mod curvebin;
pub mod dataflash;
pub mod error;
pub mod fieldstats;
pub mod format;
pub mod logfile;
pub mod query;
pub mod registry;
pub mod signal;
pub mod summary;
pub mod textrows;
pub mod tlog;
pub mod typebody;
pub mod types;
pub mod ulog;
#[cfg(target_arch = "wasm32")]
pub mod wasm_api;

use std::sync::atomic::{AtomicU32, Ordering};

use error::{ParseError, Result};
use logfile::LogFile;

/// An incremental format backend. `feed` may be called arbitrarily often with
/// arbitrary chunk sizes; record boundaries are the backend's own concern.
pub trait StreamingBackend: Send {
    /// Ingests one chunk of the raw file bytes.
    fn feed(&mut self, chunk: &[u8]) -> Result<()>;
    /// Flushes any tail state after the last chunk (EOF reached).
    fn finish(&mut self) -> Result<()>;
    /// The accumulated parse result.
    fn log(&mut self) -> &mut LogFile;
}

static GENERATION: AtomicU32 = AtomicU32::new(0);

/// Returns the next parse generation number (monotonic).
pub fn next_generation() -> u32 {
    GENERATION.fetch_add(1, Ordering::SeqCst) + 1
}

/// One parse session. Detection is deferred until enough head bytes arrive
/// (8 bytes, or immediately for the .tlog extension), mirroring Go's
/// `ParseFile` sniff-then-dispatch.
pub struct Session {
    pub generation: u32,
    filename: String,
    head: Vec<u8>,
    backend: Option<Box<dyn StreamingBackend>>,
    fed: u64,
    finished: bool,
}

impl Session {
    pub fn new(generation: u32, filename: &str) -> Session {
        Session {
            generation,
            filename: filename.to_string(),
            head: Vec::with_capacity(8),
            backend: None,
            fed: 0,
            finished: false,
        }
    }

    /// Feeds one chunk. Returns the total bytes fed so far (drives the parse
    /// progress UI).
    pub fn feed(&mut self, chunk: &[u8]) -> Result<u64> {
        if self.finished {
            return Err(ParseError::new("parse already finished"));
        }
        self.fed += chunk.len() as u64;
        if self.backend.is_none() {
            self.head.extend_from_slice(chunk);
            let decided =
                self.filename.to_ascii_lowercase().ends_with(".tlog") || self.head.len() >= 8;
            if !decided {
                return Ok(self.fed);
            }
            let Some(fmt) = registry::detect(&self.head, &self.filename) else {
                return Err(ParseError::new(format!(
                    "unrecognized log format: {}",
                    self.filename
                )));
            };
            let mut backend = create_backend(fmt, &self.filename);
            let head = std::mem::take(&mut self.head);
            backend.feed(&head)?;
            self.backend = Some(backend);
            return Ok(self.fed);
        }
        if let Some(b) = self.backend.as_mut() {
            b.feed(chunk)?;
        }
        Ok(self.fed)
    }

    /// Flushes tail state, finalizes the LogFile, and returns it.
    pub fn finish(&mut self) -> Result<&mut LogFile> {
        if self.finished {
            return self
                .backend
                .as_mut()
                .map(|b| b.log())
                .ok_or_else(|| ParseError::new("empty session"));
        }
        // A file smaller than the head window: detect with what we have.
        if self.backend.is_none() {
            let fmt = registry::detect(&self.head, &self.filename)
                .ok_or_else(|| ParseError::new("unrecognized log format"))?;
            let mut backend = create_backend(fmt, &self.filename);
            let head = std::mem::take(&mut self.head);
            backend.feed(&head)?;
            self.backend = Some(backend);
        }
        let filename = self.filename.clone();
        if let Some(b) = self.backend.as_mut() {
            // Backends own their finalize (duration + freeze), mirroring the
            // Go loaders; the session only stamps the display filename.
            b.finish()?;
            summary::apply_summary_projection(b.log());
            b.log().summary.filename = filename;
        }
        self.finished = true;
        Ok(self.backend.as_mut().expect("backend present").log())
    }
}

impl Session {
    /// Total bytes fed so far.
    pub fn fed(&self) -> u64 {
        self.fed
    }

    /// The parse filename (drives format sniffing and the summary display).
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Consumes the session, dropping the streaming backend and its buffers
    /// while keeping the finalized LogFile (the wasm layer hands out stable
    /// views into its TypeBody storage).
    pub fn into_log(mut self) -> LogFile {
        match self.backend.as_mut() {
            Some(b) => std::mem::take(b.log()),
            None => LogFile::default(),
        }
    }
}

fn create_backend(fmt: registry::LogFormat, filename: &str) -> Box<dyn StreamingBackend> {
    match fmt {
        registry::LogFormat::Ulog => Box::new(ulog::UlogBackend::new(filename)),
        registry::LogFormat::Tlog => Box::new(tlog::TlogBackend::new(filename)),
        registry::LogFormat::Dataflash => Box::new(dataflash::DataflashBackend::new(filename)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_detects_ulog_and_finalizes() {
        let mut s = Session::new(1, "test.ulg");
        let consumed = s
            .feed(&[
                0x55, 0x4C, 0x6F, 0x67, 0x01, 0x12, 0x35, 0x01, 0, 0, 0, 0, 0, 0, 0, 0,
            ])
            .unwrap();
        assert_eq!(consumed, 16);
        let lf = s.finish().unwrap();
        assert_eq!(lf.summary.format, "ulog");
    }

    #[test]
    fn session_tlog_by_extension_before_bytes() {
        let mut s = Session::new(2, "test.tlog");
        s.feed(&[0x01]).unwrap();
        let lf = s.finish().unwrap();
        assert_eq!(lf.summary.format, "tlog");
    }

    #[test]
    fn session_buffers_short_head() {
        let mut s = Session::new(3, "test.bin");
        let consumed = s.feed(&[0xA3]).unwrap();
        assert_eq!(consumed, 1);
        s.feed(&[0x95, 0x80, 1, 2, 3, 4, 5]).unwrap(); // now 8 bytes
        let lf = s.finish().unwrap();
        assert_eq!(lf.summary.format, "apm");
    }
}
