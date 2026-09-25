//! ArduPilot DataFlash backend (.bin binary / .log text), ported from
//! `app/modules/parser/dataflash/loader.go`. The loader's whole-file run()
//! becomes an incremental mode decision on the first bytes followed by one of
//! the two streaming scanners.

pub mod binary_scan;
pub mod dispatch;
pub mod fielddecode;
pub mod modetable;
pub mod schema;
pub mod text_scan;
pub mod timebase;

use crate::error::Result;
use crate::logfile::LogFile;
use crate::StreamingBackend;

/// DataFlash magic prefix: every binary record begins with 0xA3 0x95.
pub const MAGIC_HI: u8 = 0xA3;
pub const MAGIC_LO: u8 = 0x95;
pub const FMT_MSG_ID: u8 = 0x80;

enum Mode {
    /// Not enough bytes yet to pick binary-vs-text.
    Undecided,
    Text(text_scan::TextScanner),
    Binary(binary_scan::BinaryScanner),
}

/// The ArduPilot DataFlash fallback backend. Claims any stream that is
/// neither a ULog nor a MAVLink capture (the session layer guarantees that).
pub struct DataflashBackend {
    log: LogFile,
    mode: Mode,
    /// Bytes to skip at the stream start (the 4-byte UTC-prefix variant).
    binary_skip: usize,
    fed: u64,
}

impl DataflashBackend {
    pub fn new(filename: &str) -> DataflashBackend {
        DataflashBackend {
            log: LogFile::new(filename, "apm"),
            mode: Mode::Undecided,
            binary_skip: 0,
            fed: 0,
        }
    }

    /// Mirrors Go run(): binary at offset 0, binary at offset 4 behind a
    /// UTC-seconds prefix, otherwise text. Sets `skip` for the offset-4
    /// variant.
    fn decide(&mut self, first: &[u8], skip: &mut usize) -> Mode {
        if first.len() >= 2 && first[0] == MAGIC_HI && first[1] == MAGIC_LO {
            return Mode::Binary(binary_scan::BinaryScanner::new(0, &mut self.log));
        }
        if first.len() >= 6 && first[4] == MAGIC_HI && first[5] == MAGIC_LO {
            let utc = if first.len() >= 4 {
                u32::from_le_bytes([first[0], first[1], first[2], first[3]])
            } else {
                0
            };
            *skip = 4;
            return Mode::Binary(binary_scan::BinaryScanner::new(utc, &mut self.log));
        }
        let mut s = text_scan::TextScanner::new();
        s.begin(&mut self.log);
        Mode::Text(s)
    }
}

impl StreamingBackend for DataflashBackend {
    fn feed(&mut self, chunk: &[u8]) -> Result<()> {
        self.fed += chunk.len() as u64;
        let mut chunk = chunk;
        if let Mode::Undecided = self.mode {
            let mut skip = 0usize;
            self.binary_skip = 0;
            self.mode = self.decide(chunk, &mut skip);
            self.binary_skip = skip;
        }
        if self.binary_skip > 0 {
            let skip = self.binary_skip.min(chunk.len());
            self.binary_skip -= skip;
            chunk = &chunk[skip..];
        }
        if chunk.is_empty() {
            return Ok(());
        }
        match &mut self.mode {
            Mode::Text(s) => s.feed(&mut self.log, chunk),
            Mode::Binary(s) => s.feed(&mut self.log, chunk),
            Mode::Undecided => Ok(()),
        }
    }

    fn finish(&mut self) -> Result<()> {
        if let Mode::Undecided = self.mode {
            let mut skip = 0usize;
            self.mode = self.decide(&[], &mut skip);
            self.binary_skip = skip;
        }
        match &mut self.mode {
            Mode::Text(s) => s.finish(&mut self.log)?,
            Mode::Binary(s) => s.finish(&mut self.log)?,
            Mode::Undecided => {}
        }
        self.log.set_file_size_kb(self.fed as f64 / 1024.0);
        self.log.derive_duration(&["GPS", "GPS1"]);
        self.log.finalize();
        Ok(())
    }

    fn log(&mut self) -> &mut LogFile {
        &mut self.log
    }
}
