//! Format registry and magic detection, ported from
//! `app/modules/parser/parse.go` minus the Go interface plumbing: backends are
//! compile-time modules selected by `detect`, keeping the "formats only get
//! added, dispatch never changes" invariant.

/// The 7-byte magic prefix that opens every ULog file.
pub const ULOG_FILE_HEADER: [u8; 7] = [0x55, 0x4C, 0x6F, 0x67, 0x01, 0x12, 0x35];

/// Reports whether head begins with the ULog magic.
pub fn is_ulog(head: &[u8]) -> bool {
    head.len() >= 7 && head[..7] == ULOG_FILE_HEADER
}

/// Reports whether the file looks like a MAVLink stream — either a .tlog
/// extension or a v1/v2 packet start byte (0xFE / 0xFD).
pub fn is_mavlink(head: &[u8], filename: &str) -> bool {
    if filename.to_ascii_lowercase().ends_with(".tlog") {
        return true;
    }
    !head.is_empty() && (head[0] == 0xFD || head[0] == 0xFE)
}

/// A detected log format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Ulog,
    Tlog,
    Dataflash,
}

impl LogFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            LogFormat::Ulog => "ulog",
            LogFormat::Tlog => "tlog",
            LogFormat::Dataflash => "apm",
        }
    }
}

/// Sniffs the log format from the file head and filename. Mirrors the Go
/// loader precedence: ulog and tlog match on precise magic/extension, the
/// dataflash backend is the negative fallback.
pub fn detect(head: &[u8], filename: &str) -> Option<LogFormat> {
    if is_ulog(head) {
        return Some(LogFormat::Ulog);
    }
    if is_mavlink(head, filename) {
        return Some(LogFormat::Tlog);
    }
    Some(LogFormat::Dataflash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ulog_magic() {
        assert_eq!(detect(&ULOG_FILE_HEADER, "x.ulg"), Some(LogFormat::Ulog));
        assert!(is_ulog(&[0x55, 0x4C, 0x6F, 0x67, 0x01, 0x12, 0x35, 0x01]));
    }

    #[test]
    fn detects_tlog() {
        assert_eq!(detect(&[0xFD, 0x09, 0x00], "x.bin"), Some(LogFormat::Tlog));
        assert_eq!(detect(&[0xFE], "x.bin"), Some(LogFormat::Tlog));
        assert_eq!(detect(&[0x00, 0x01], "x.TLOG"), Some(LogFormat::Tlog));
    }

    #[test]
    fn dataflash_is_fallback() {
        assert_eq!(detect(b"FMT, 1", "x.log"), Some(LogFormat::Dataflash));
        assert_eq!(
            detect(&[0xA3, 0x95, 0x80], "x.bin"),
            Some(LogFormat::Dataflash)
        );
    }
}
