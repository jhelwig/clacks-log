//! Strict inventory and bounded ZIP expansion into a private temporary directory.

use crate::{
    recipe::{SourcePin, validate_relative},
    source::check_cancelled,
};
use anyhow::{Context, ensure};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    sync::atomic::AtomicBool,
};

/// Expand only the pinned member inventory, counting actual decompressed bytes.
pub(super) fn extract(
    pin: &SourcePin,
    path: &Path,
    cancelled: &AtomicBool,
) -> anyhow::Result<tempfile::TempDir> {
    let mut archive = zip::ZipArchive::new(File::open(path)?)?;
    archive.validate_header_consistency()?;
    ensure!(
        archive.len() <= pin.limits.members,
        "archive entry limit exceeded"
    );
    let expected: BTreeSet<_> = pin.archive_members.iter().collect();
    let mut seen = BTreeSet::new();
    let directory = tempfile::tempdir()?;
    let mut total = 0u64;
    for index in 0..archive.len() {
        check_cancelled(cancelled)?;
        let mut member = archive.by_index(index)?;
        let name = member.name()?.into_owned();
        validate_relative(&name)?;
        ensure!(
            !member.is_dir()
                && member
                    .unix_mode()
                    .is_none_or(|mode| matches!(mode & 0o170000, 0 | 0o100000)),
            "nonregular archive member"
        );
        ensure!(
            expected.contains(&name) && seen.insert(name.clone()),
            "unlisted or duplicate archive member {name}"
        );
        ensure!(
            member.size() <= pin.limits.member_bytes,
            "declared member size exceeds limit"
        );
        let target = directory.path().join(&name);
        fs::create_dir_all(target.parent().context("archive member parent")?)?;
        let mut output = File::create(target)?;
        let mut bytes = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            check_cancelled(cancelled)?;
            let count = member.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            bytes = bytes
                .checked_add(count as u64)
                .context("member size overflow")?;
            total = total
                .checked_add(count as u64)
                .context("archive size overflow")?;
            ensure!(
                bytes <= pin.limits.member_bytes && total <= pin.limits.expanded_bytes,
                "actual archive expansion exceeds limit"
            );
            output.write_all(buffer.get(..count).context("read buffer length")?)?;
        }
        ensure!(
            bytes == member.size(),
            "archive member length differs from declaration"
        );
    }
    ensure!(
        seen.iter().collect::<BTreeSet<_>>() == expected,
        "archive missing declared member"
    );
    Ok(directory)
}
