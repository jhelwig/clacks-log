//! Ordinary PostgreSQL snapshot tools for an explicitly isolated restore scenario.

use sqlx::postgres::PgConnectOptions;
use std::{
    fs::File,
    path::Path,
    process::{Command, Stdio},
    str::FromStr,
};

/// Explicitly selected disposable database; never inferred from a development setup.
pub(super) struct Database {
    container: String,
    database: String,
    user: String,
}

impl Database {
    /// Require the operator to name the exclusively owned test container.
    pub(super) fn selected() -> anyhow::Result<Self> {
        let container = std::env::var("CLACKS_RESTORE_CONTAINER")?;
        anyhow::ensure!(
            container.starts_with("clacks-retention-test"),
            "restore requires an owned retention-test container"
        );

        let options = PgConnectOptions::from_str(&std::env::var("CLACKS_TEST_DATABASE_URL")?)?;

        Ok(Self {
            container,
            database: options
                .get_database()
                .ok_or_else(|| anyhow::anyhow!("test database must be explicit"))?
                .into(),
            user: options.get_username().into(),
        })
    }

    /// Capture a closed server's actual database using PostgreSQL's custom archive format.
    pub(super) fn snapshot(&self, path: &Path) -> anyhow::Result<()> {
        let output = Command::new("docker")
            .args([
                "exec",
                &self.container,
                "pg_dump",
                "-U",
                &self.user,
                "--format=custom",
                "--dbname",
                &self.database,
            ])
            .stdout(File::create(path)?)
            .stderr(Stdio::piped())
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "pg_dump failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        Ok(())
    }

    /// Replace only this isolated database while every fixture server connection is closed.
    pub(super) fn restore(&self, path: &Path) -> anyhow::Result<()> {
        let output = Command::new("docker")
            .args([
                "exec",
                "-i",
                &self.container,
                "pg_restore",
                "-U",
                &self.user,
                "--dbname",
                &self.database,
                "--clean",
                "--if-exists",
                "--no-owner",
                "--exit-on-error",
                "--single-transaction",
            ])
            .stdin(File::open(path)?)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "pg_restore failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        Ok(())
    }
}
