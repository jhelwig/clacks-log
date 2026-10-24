//! Standard writer I/O boundary for persistent file-failure diagnostics and stderr fallback.

use std::{
    io::{self, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Forward output to the rotating file until failure, then use the standard fallback.
///
/// Rotation, queuing, formatting and worker lifetime remain owned by the existing
/// appender. The failure flag is sticky for this process; lost bytes are not replayed.
pub(super) struct FallbackWriter<W, F> {
    /// File sink retained while writes remain usable.
    primary: Option<W>,
    /// Standard stderr output after the file fails.
    fallback: F,
    /// Shared non-secret failure indication for the running UI.
    failed: Arc<AtomicBool>,
}

impl<W, F> FallbackWriter<W, F> {
    /// Attach standard output sinks and a failure indication, without spawning work.
    pub fn new(primary: W, fallback: F, failed: Arc<AtomicBool>) -> Self {
        Self {
            primary: Some(primary),
            fallback,
            failed,
        }
    }
}

impl<W: Write, F: Write> Write for FallbackWriter<W, F> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(primary) = &mut self.primary {
            match primary.write(bytes) {
                Ok(written) if written != 0 || bytes.is_empty() => return Ok(written),
                Ok(_) | Err(_) => {
                    self.failed.store(true, Ordering::Relaxed);
                    self.primary = None;
                }
            }
        }
        self.fallback.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(primary) = &mut self.primary {
            match primary.flush() {
                Ok(()) => return Ok(()),
                Err(_) => {
                    self.failed.store(true, Ordering::Relaxed);
                    self.primary = None;
                }
            }
        }
        self.fallback.flush()
    }
}
