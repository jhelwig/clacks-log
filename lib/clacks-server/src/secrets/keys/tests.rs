//! Account-key ownership and authenticated deployment-key references.

use super::*;

#[test]
fn wrapping_key_labels_are_authenticated_even_when_key_material_is_reused() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("wrapping-key");
    std::fs::write(&file, [19; 32])?;
    let wrapping = WrappingKeys::load(&WrappingKeyConfig {
        active: "release$旧".into(),
        files: [
            ("release$旧".into(), file.clone()),
            ("replacement".into(), file),
        ]
        .into(),
    })?;
    let account = AccountId::new();
    let encoded = wrap(&wrapping, account, 1, &[11; 32])?.encode();
    let restored = unwrap_key(
        &wrapping,
        account,
        KeyRow {
            key_version: 1,
            wrapped_envelope: encoded.clone(),
        },
    )?;
    assert_eq!(restored.bytes.as_slice(), &[11; 32]);
    let changed = encoded.replacen(
        &envelope::wrapping_reference("release$旧"),
        &envelope::wrapping_reference("replacement"),
        1,
    );
    assert!(
        unwrap_key(
            &wrapping,
            account,
            KeyRow {
                key_version: 1,
                wrapped_envelope: changed
            }
        )
        .is_err()
    );
    assert!(
        unwrap_key(
            &wrapping,
            AccountId::new(),
            KeyRow {
                key_version: 1,
                wrapped_envelope: encoded
            }
        )
        .is_err()
    );
    Ok(())
}
