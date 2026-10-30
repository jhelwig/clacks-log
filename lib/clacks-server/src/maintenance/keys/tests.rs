//! Shared exclusion for tests invoking deployment-wide key maintenance.

pub(crate) static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
