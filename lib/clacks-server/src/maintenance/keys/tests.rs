//! Shared exclusion for tests invoking deployment-wide key maintenance.

// Keep callers in the `server-key-maintenance` group in `.config/nextest.toml`.
// This mutex only coordinates tests within one process.
pub(crate) static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
