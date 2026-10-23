//! Digest-keyed source storage and explicit bounded network acquisition.

use super::check_cancelled;
use crate::recipe::SourcePin;
use anyhow::{Context, ensure};
use sha2_rsa::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};

/// Verify a regular cached blob before decoding it; presence is not integrity.
pub(super) fn verify(pin: &SourcePin, cache: &Path) -> anyhow::Result<PathBuf> {
    let path = cache.join(format!("{}.blob", pin.archive_sha256));
    let metadata = fs::symlink_metadata(&path)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "cache entry is not a regular file"
    );
    ensure!(
        metadata.len() <= pin.limits.download_bytes,
        "cached source exceeds download limit"
    );
    let mut file = File::open(&path)?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest)?;
    ensure!(
        hex::encode(digest.finalize()) == pin.archive_sha256,
        "cached source digest mismatch"
    );
    Ok(path)
}

/// Download and install one verified source without overwriting a cache race winner.
#[tracing::instrument(skip_all,name="geography.source.fetch",level="info",fields(source_id = %pin.id, expected_digest = %pin.archive_sha256))]
pub(super) fn fetch(pin: &SourcePin, cache: &Path, cancelled: &AtomicBool) -> anyhow::Result<bool> {
    let destination = cache.join(format!("{}.blob", pin.archive_sha256));
    match fs::symlink_metadata(&destination) {
        Ok(_) => {
            verify(pin, cache)?;
            return Ok(false);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    check_cancelled(cancelled)?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(300)))
        .timeout_connect(Some(Duration::from_secs(30)))
        .build()
        .into();
    let mut response = agent
        .get(&pin.download_url)
        .call()
        .context("source HTTP request failed")?;
    let mut reader = response.body_mut().as_reader();
    let mut temporary = tempfile::NamedTempFile::new_in(cache)?;
    let mut buffer = [0u8; 65536];
    let mut count = 0u64;
    let mut digest = Sha256::new();
    loop {
        check_cancelled(cancelled)?;
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        count = count
            .checked_add(read as u64)
            .context("download length overflow")?;
        ensure!(
            count <= pin.limits.download_bytes,
            "source download exceeds configured limit"
        );
        let bytes = buffer.get(..read).context("read buffer length")?;
        temporary.write_all(bytes)?;
        digest.update(bytes);
    }
    ensure!(
        hex::encode(digest.finalize()) == pin.archive_sha256,
        "downloaded source digest mismatch"
    );
    check_cancelled(cancelled)?;
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(&destination) {
        Ok(_) => Ok(true),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            verify(pin, cache)?;
            Ok(false)
        }
        Err(error) => Err(error.error.into()),
    }
}
