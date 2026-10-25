//! Current-user Unix endpoint permissions and stable lock-file identity.

#[cfg(test)]
#[path = "unix_tests.rs"]
mod tests;

use super::{ActivationAddress, suffix};
use directories::ProjectDirs;
use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};
use std::{
    fs::{DirBuilder, File},
    io,
    os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
    path::PathBuf,
};

/// Choose a private, bounded-length directory for Unix socket names.
fn endpoint_directory() -> anyhow::Result<PathBuf> {
    // SAFETY: geteuid has no input preconditions and does not retain pointers.
    let uid = unsafe { libc::geteuid() };
    let directory = ProjectDirs::from("", "", "Clacks Log")
        .and_then(|project| project.runtime_dir().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/tmp").join(format!("clacks-log-{uid}")));

    match DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory)
    {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }

    verify_directory(&directory, uid)?;
    Ok(directory)
}

/// Check actual endpoint-directory ownership and permissions without following symlinks.
fn verify_directory(directory: &std::path::Path, uid: u32) -> anyhow::Result<()> {
    let metadata = directory.symlink_metadata()?;
    anyhow::ensure!(
        metadata.is_dir() && metadata.uid() == uid && metadata.permissions().mode() & 0o077 == 0,
        "activation directory must be a private directory owned by the current user"
    );
    Ok(())
}

/// Derive an endpoint from actual lock inode/device and current user identity.
pub(super) fn address(file: &File) -> anyhow::Result<ActivationAddress> {
    let metadata = file.metadata()?;
    // SAFETY: geteuid has no input preconditions.
    let uid = unsafe { libc::geteuid() };
    let identity = format!("{}:{}:{uid}", metadata.dev(), metadata.ino());
    let path = endpoint_directory()?.join(format!("{}.sock", suffix(identity.as_bytes())));
    let name = path.clone().to_fs_name::<GenericFilePath>()?;
    Ok(ActivationAddress { name, path })
}

/// Recover only this owner's stale socket and prepare its endpoint name.
pub(super) fn owned_options(
    address: &ActivationAddress,
) -> anyhow::Result<ListenerOptions<'static>> {
    match address.path.symlink_metadata() {
        Ok(metadata) => {
            anyhow::ensure!(
                metadata.file_type().is_socket(),
                "activation endpoint is occupied by a non-socket file"
            );
            std::fs::remove_file(&address.path)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    Ok(ListenerOptions::new().name(address.name.clone()))
}

/// Bind a user-restricted endpoint using the platform-supported permission operation.
pub(super) fn bind(
    address: &ActivationAddress,
) -> anyhow::Result<interprocess::local_socket::tokio::Listener> {
    #[cfg(not(target_os = "macos"))]
    let listener = {
        use interprocess::os::unix::local_socket::ListenerOptionsExt;
        owned_options(address)?.mode(0o600).create_tokio()?
    };

    #[cfg(target_os = "macos")]
    let listener = {
        let listener = owned_options(address)?.create_tokio()?;
        // The validated 0700 parent excludes other users throughout this operation.
        // macOS does not support interprocess's pre-bind socket fchmod operation.
        std::fs::set_permissions(&address.path, std::fs::Permissions::from_mode(0o600))?;
        listener
    };

    Ok(listener)
}
