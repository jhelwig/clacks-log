//! One-process workspace ownership, independent of SQLite locking.

pub(crate) mod activation;
#[cfg(test)]
pub(crate) mod tests;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use crate::workspace::WorkspacePaths;
use interprocess::local_socket::Name;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

/// Address of the owner, without permission to displace its endpoint.
#[derive(Clone)]
pub(crate) struct ActivationAddress {
    /// Owned cross-platform local socket name.
    pub name: Name<'static>,
    /// Unix filesystem socket path used only by owner-controlled cleanup.
    #[cfg(unix)]
    pub path: std::path::PathBuf,
}

/// Result of acquiring actual OS ownership, distinct from file existence.
pub(crate) enum Election {
    /// This process may initialize and serve the selected workspace.
    Owner(InstanceOwner),
    /// An existing owner must receive activation; no second store may open.
    Contended(ActivationAddress),
}

/// OS-held lifetime ownership, retained until application cleanup finishes.
pub(crate) struct InstanceOwner {
    /// Keep the original locked handle alive; never unlink its sidecar.
    _lock: File,
    /// Workspace-qualified activation endpoint.
    pub address: ActivationAddress,
}

impl InstanceOwner {
    /// Bind the owned activation endpoint before lengthy workspace initialization.
    pub fn listen(
        &self,
        runtime: &tokio::runtime::Handle,
        notices: tokio::sync::watch::Sender<()>,
    ) -> anyhow::Result<activation::ActivationListener> {
        let listener = {
            let _entered = runtime.enter();
            platform::bind(&self.address)?
        };

        Ok(activation::listen(listener, runtime, notices))
    }
}

/// Elect one owner using an OS lifetime lock, leaving database files untouched.
pub(crate) fn elect(paths: &WorkspacePaths) -> anyhow::Result<Election> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&paths.lock_file)?;

    match file.try_lock() {
        Ok(()) => {
            let address = platform::address(&file)?;
            Ok(Election::Owner(InstanceOwner {
                _lock: file,
                address,
            }))
        }
        Err(std::fs::TryLockError::WouldBlock) => {
            Ok(Election::Contended(platform::address(&file)?))
        }
        Err(std::fs::TryLockError::Error(error)) => Err(error.into()),
    }
}

/// Build a bounded endpoint suffix from platform identity, never lossy path text.
fn suffix(identity: &[u8]) -> String {
    hex::encode(Sha256::digest(identity))
}
