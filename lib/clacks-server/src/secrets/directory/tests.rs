//! Tests Clacks's owner/purpose/identity/revision binding, not the cipher itself.

use super::*;
use clacks_sync::protocol::directory::DirectoryLogin;

#[test]
fn directory_envelope_authenticates_owner_purpose_identity_and_revision() -> anyhow::Result<()> {
    let account = "00000000000000000000000001".parse()?;
    let material = DirectoryMaterial {
        id: "00000000000000000000000002".parse()?,
        revision: 1,
        login: DirectoryLogin {
            username: "fixture".into(),
            password: "secret-sentinel".into(),
        },
    };
    let key = AccountKey {
        account,
        version: 1,
        bytes: Zeroizing::new([17; 32]),
    };
    let expected = DirectoryContext {
        account,
        provider: DirectoryProvider::Qrz,
        material_id: material.id,
        material_revision: 1,
    };
    let encoded = seal(&key, &expected, &material)?.encode();
    let envelope = DirectoryEnvelope::parse(&encoded)?;
    for wrong in [
        DirectoryContext {
            provider: DirectoryProvider::HamQth,
            ..expected
        },
        DirectoryContext {
            material_id: "00000000000000000000000003".parse()?,
            ..expected
        },
        DirectoryContext {
            material_revision: 2,
            ..expected
        },
    ] {
        assert!(open(&key, &wrong, &envelope).is_err());
    }
    let other_account = DirectoryContext {
        account: "00000000000000000000000004".parse()?,
        ..expected
    };
    let other_key = AccountKey {
        account: other_account.account,
        version: 1,
        bytes: Zeroizing::new([17; 32]),
    };
    assert!(open(&other_key, &other_account, &envelope).is_err());
    assert_eq!(open(&key, &expected, &envelope)?, material);
    assert!(!format!("{material:?}").contains("secret-sentinel"));
    Ok(())
}
