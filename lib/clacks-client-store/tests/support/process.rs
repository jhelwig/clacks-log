//! Test-binary interruption protocol; no production helper executable is involved.

use std::{
    path::Path,
    process::{Child, Command},
    time::{Duration, Instant},
};

pub struct InterruptedChild(Child);

impl InterruptedChild {
    pub fn start(test: &str, file: &Path, phase: &str) -> anyhow::Result<Self> {
        Ok(Self(
            Command::new(std::env::current_exe()?)
                .args(["--exact", test, "--ignored", "--nocapture"])
                .env("CLACKS_TEST_DATABASE", file)
                .env("CLACKS_TEST_PHASE", phase)
                .spawn()?,
        ))
    }

    pub fn wait_ready(&mut self, ready: &Path) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !ready.exists() {
            if let Some(status) = self.0.try_wait()? {
                anyhow::bail!("child exited before its interruption boundary: {status}");
            }
            anyhow::ensure!(
                Instant::now() < deadline,
                "child did not reach its interruption boundary"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }

    pub fn kill(&mut self) -> anyhow::Result<()> {
        self.0.kill()?;
        self.0.wait()?;
        Ok(())
    }
}

impl Drop for InterruptedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
