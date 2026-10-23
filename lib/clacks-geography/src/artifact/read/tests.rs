//! Filesystem error-source regression cases for integrity validation.

use super::*;
use std::error::Error;

#[test]
fn verification_preserves_a_read_error_source() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("write-only");
    let mut file = File::options()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    let expected = crate::artifact::FileDigest {
        path: "write-only".into(),
        bytes: 0,
        sha256: "0".repeat(64),
    };
    let error = verify_file(&mut file, &expected).err().unwrap();
    assert!(
        error.source().is_some(),
        "integrity verification erased the I/O error source"
    );
}

#[cfg(unix)]
#[test]
fn nonregular_entries_are_rejected_before_opening() {
    let directory = tempfile::tempdir().unwrap();
    let _socket =
        std::os::unix::net::UnixListener::bind(directory.path().join("manifest.json")).unwrap();
    assert!(matches!(
        open_regular(directory.path(), "manifest.json"),
        Err(GeographyError::InvalidArtifact { .. })
    ));
}
