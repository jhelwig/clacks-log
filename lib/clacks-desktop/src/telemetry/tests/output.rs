//! Clacks's sink adapter keeps failure sticky and forwards later output to fallback.

use super::super::output::FallbackWriter;
use std::{
    io::{self, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Default)]
struct State {
    bytes: Vec<u8>,
    flushes: usize,
    fail_write: bool,
    fail_flush: bool,
}

struct Sink(Arc<Mutex<State>>);

impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut sink = self.0.lock().unwrap();
        if sink.fail_write {
            return Err(io::ErrorKind::StorageFull.into());
        }
        sink.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut sink = self.0.lock().unwrap();
        sink.flushes += 1;
        if sink.fail_flush {
            Err(io::ErrorKind::StorageFull.into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn write_failure_forwards_current_and_later_bytes_to_fallback() -> io::Result<()> {
    let primary = Arc::new(Mutex::new(State::default()));
    let fallback = Arc::new(Mutex::new(State::default()));
    let failed = Arc::new(AtomicBool::new(false));
    let mut writer = FallbackWriter::new(
        Sink(primary.clone()),
        Sink(fallback.clone()),
        failed.clone(),
    );
    writer.write_all(b"before")?;
    writer.flush()?;
    assert!(!failed.load(Ordering::Relaxed));
    assert_eq!(primary.lock().unwrap().bytes, b"before");
    assert_eq!(primary.lock().unwrap().flushes, 1);
    assert!(fallback.lock().unwrap().bytes.is_empty());

    primary.lock().unwrap().fail_write = true;
    writer.write_all(b"during")?;
    primary.lock().unwrap().fail_write = false;
    writer.write_all(b"after")?;
    writer.flush()?;

    assert!(failed.load(Ordering::Relaxed));
    assert_eq!(primary.lock().unwrap().bytes, b"before");
    assert_eq!(fallback.lock().unwrap().bytes, b"duringafter");
    assert_eq!(fallback.lock().unwrap().flushes, 1);
    Ok(())
}

#[test]
fn flush_failure_switches_to_fallback_and_propagates_its_error() -> io::Result<()> {
    let primary = Arc::new(Mutex::new(State {
        fail_flush: true,
        ..Default::default()
    }));
    let fallback = Arc::new(Mutex::new(State::default()));
    let failed = Arc::new(AtomicBool::new(false));
    let mut writer = FallbackWriter::new(
        Sink(primary.clone()),
        Sink(fallback.clone()),
        failed.clone(),
    );
    writer.flush()?;
    writer.write_all(b"after flush failure")?;
    assert!(failed.load(Ordering::Relaxed));
    assert_eq!(primary.lock().unwrap().flushes, 1);
    assert_eq!(fallback.lock().unwrap().flushes, 1);
    assert_eq!(fallback.lock().unwrap().bytes, b"after flush failure");

    fallback.lock().unwrap().fail_write = true;
    assert_eq!(
        writer.write(b"lost").unwrap_err().kind(),
        io::ErrorKind::StorageFull
    );
    fallback.lock().unwrap().fail_flush = true;
    assert_eq!(
        writer.flush().unwrap_err().kind(),
        io::ErrorKind::StorageFull
    );
    Ok(())
}
