//! Clacks binds each receipt to its owning account, code and approval purpose.

use super::*;

#[test]
fn receipt_authenticates_account_code_and_purpose() -> anyhow::Result<()> {
    let key = AccountKey {
        account: AccountId::new(),
        version: 1,
        bytes: Zeroizing::new([17; 32]),
    };
    let context = ReceiptContext {
        account: key.account,
        code_digest: [23; 32],
        purpose: "new".into(),
    };
    let references = IssuedReferences {
        access_token: Some("access-secret".into()),
        refresh_token: Some("refresh-secret".into()),
    };

    let stored = seal(&key, &context, &references)?;
    let decoded = open(&key, &context, &stored)?;

    assert_eq!(decoded.access_token, references.access_token);
    assert_eq!(decoded.refresh_token, references.refresh_token);

    for changed in [
        ReceiptContext {
            account: AccountId::new(),
            code_digest: context.code_digest,
            purpose: context.purpose.clone(),
        },
        ReceiptContext {
            account: context.account,
            code_digest: [24; 32],
            purpose: context.purpose.clone(),
        },
        ReceiptContext {
            account: context.account,
            code_digest: context.code_digest,
            purpose: "replacement".into(),
        },
    ] {
        assert!(open(&key, &changed, &stored).is_err());
    }

    Ok(())
}
