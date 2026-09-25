//! PX4 ULog backend (.ulg), ported from `app/modules/parser/ulog/loader.go`.
//! The whole-file decode becomes incremental over length-prefixed records:
//! an incomplete record simply waits for more bytes, and a truncated tail is
//! dropped at finish exactly like the Go loop's `break`.

pub mod events;
pub mod fields;
pub mod mission;
pub mod records;
pub mod schema;
pub mod timebase;
pub mod values;

use crate::error::{ParseError, Result};
use crate::logfile::LogFile;
use crate::StreamingBackend;

/// The 7-byte magic that prefixes every well-formed ULog.
pub const FILE_SIGNATURE: [u8; 7] = [0x55, 0x4C, 0x6F, 0x67, 0x01, 0x12, 0x35];

/// ULog file header: magic(7) + version(1) + timestamp(8).
const FILE_HEADER: usize = 16;

pub struct UlogBackend {
    session: records::DecodeSession,
    buf: Vec<u8>,
    header_ok: bool,
    fed: u64,
}

impl UlogBackend {
    pub fn new(filename: &str) -> UlogBackend {
        UlogBackend {
            session: records::DecodeSession::new(filename),
            buf: Vec::new(),
            header_ok: false,
            fed: 0,
        }
    }

    /// Validates the 16-byte file header once enough bytes are buffered.
    fn validate_header(&mut self) -> Result<()> {
        if self.header_ok {
            return Ok(());
        }
        if self.buf.len() < FILE_HEADER {
            return Ok(()); // wait for more bytes
        }
        if self.buf[..7] != FILE_SIGNATURE {
            return Err(ParseError::new("ulog: bad magic"));
        }
        self.buf.drain(..FILE_HEADER);
        self.header_ok = true;
        Ok(())
    }
}

impl StreamingBackend for UlogBackend {
    fn feed(&mut self, chunk: &[u8]) -> Result<()> {
        self.fed += chunk.len() as u64;
        self.buf.extend_from_slice(chunk);
        self.validate_header()?;
        if !self.header_ok {
            return Ok(());
        }
        let mut off = 0usize;
        while off + 3 <= self.buf.len() {
            let rec_len = u16::from_le_bytes([self.buf[off], self.buf[off + 1]]) as usize;
            let rec_type = self.buf[off + 2];
            let p_start = off + 3;
            if p_start + rec_len > self.buf.len() {
                break; // incomplete record: wait for more bytes
            }
            self.session.next_record_no();
            self.session
                .dispatch_record(rec_type, &self.buf[p_start..p_start + rec_len]);
            off = p_start + rec_len;
        }
        if off > 0 {
            self.buf.drain(..off);
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        if !self.header_ok {
            if self.buf.len() < FILE_HEADER {
                return Err(ParseError::new("ulog: file too short"));
            }
            return Err(ParseError::new("ulog: bad magic"));
        }
        self.session.finish_stream();
        self.session.file.set_file_size_kb(self.fed as f64 / 1024.0);
        self.session.file.finalize();
        Ok(())
    }

    fn log(&mut self) -> &mut LogFile {
        &mut self.session.file
    }
}
