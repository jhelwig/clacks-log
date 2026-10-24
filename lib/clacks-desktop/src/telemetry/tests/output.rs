//! Runtime sink failure degrades diagnostics while real account work continues.

use super::super::output::FallbackWriter;
use std::{
    io::{self, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

struct Primary {
    fail: Arc<AtomicBool>,
    written: mpsc::Sender<()>,
}

impl Write for Primary {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail.load(Ordering::Relaxed) {
            return Err(io::ErrorKind::StorageFull.into());
        }
        let _ = self.written.send(());
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Fallback(Arc<Mutex<Vec<u8>>>);

impl Write for Fallback {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn runtime_write_failure_is_visible_and_preserves_account_work() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let paths = crate::workspace::WorkspacePaths {
        directory: directory.path().into(),
        database: directory.path().join("workspace.sqlite"),
        lock_file: directory.path().join("instance.lock"),
        log_directory: directory.path().join("logs"),
    };
    let fail = Arc::new(AtomicBool::new(false));
    let degraded = Arc::new(AtomicBool::new(false));
    let captured = Arc::new(Mutex::new(Vec::new()));
    let (written, first_write) = mpsc::channel();
    let (writer, guard) = tracing_appender::non_blocking(FallbackWriter::new(
        Primary {
            fail: fail.clone(),
            written,
        },
        Fallback(captured.clone()),
        degraded.clone(),
    ));
    let queue_loss = writer.error_counter();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_writer(writer)
        .finish();
    tracing::subscriber::with_default(subscriber, || -> anyhow::Result<()> {
        tracing::info!(event = "test.initial_output");
        first_write.recv_timeout(Duration::from_secs(2))?;
        assert!(!degraded.load(Ordering::Relaxed));
        fail.store(true, Ordering::Relaxed);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        runtime.block_on(super::operations::save_contact(&paths))?;
        Ok(())
    })?;
    drop(guard);
    assert!(degraded.load(Ordering::Relaxed));
    assert_eq!(queue_loss.dropped_lines(), 0);
    let contents = String::from_utf8(captured.lock().unwrap().clone())?;
    assert!(contents.contains("workspace.created"));
    assert!(!contents.contains("SECRET_SENTINEL"));
    assert!(paths.database.exists());
    Ok(())
}
