//! Account-scoped receipt protection through actual PostgreSQL persistence.

use crate::{
    auth::oidc::VerifiedIdentity,
    secrets::{keys, receipt},
    store::AccountStore,
};
use sqlx::postgres::PgPoolOptions;
use time::Duration;

#[tokio::test]
async fn receipt_is_account_bound_and_survives_store_reopen() -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&std::env::var("CLACKS_TEST_DATABASE_URL")?)
        .await?;
    crate::maintenance::migrate::run(&pool).await?;
    let store = AccountStore::new(pool.clone());
    let mut accounts = Vec::new();
    for _ in 0..2 {
        let login = store
            .provision_login(
                VerifiedIdentity {
                    issuer: "https://receipt-provider.example".into(),
                    subject: ulid::Ulid::generate().to_string(),
                    display_name: None,
                },
                Duration::days(14),
            )
            .await?;
        let principal = store
            .authenticate(&login, Duration::days(14))
            .await?
            .ok_or_else(|| anyhow::anyhow!("new login was not authenticated"))?;
        accounts.push(principal.account_id);
    }
    let mut accounts = accounts.into_iter();
    let account = accounts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing account"))?;
    let other = accounts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing second account"))?;
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("wrapping-key");
    std::fs::write(&file, [17u8; 32])?;
    let config = crate::config::WrappingKeyConfig {
        active: "test".into(),
        files: [("test".into(), file)].into(),
    };
    let wrapping = keys::WrappingKeys::load(&config)?;
    let key = keys::account_key(&pool, &wrapping, account).await?;
    let context = receipt::ReceiptContext {
        account,
        code_digest: rand::random(),
        purpose: "new".into(),
    };
    let references = receipt::IssuedReferences {
        access_token: Some("access-secret".into()),
        refresh_token: Some("refresh-secret".into()),
    };
    let encrypted = receipt::seal(&key, &context, &references)?;
    sqlx::query_file!(
        "tests/sql/store_receipt.sql",
        context.code_digest.as_slice(),
        account.to_string(),
        encrypted.key_version,
        encrypted.nonce,
        encrypted.ciphertext
    )
    .execute(&pool)
    .await?;
    drop(key);
    drop(wrapping);
    pool.close().await;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&std::env::var("CLACKS_TEST_DATABASE_URL")?)
        .await?;
    let restored = keys::WrappingKeys::load(&config)?;
    let stored = sqlx::query_file_as!(
        receipt::EncryptedReceipt,
        "tests/sql/load_receipt.sql",
        context.code_digest.as_slice()
    )
    .fetch_one(&pool)
    .await?;
    let key = keys::load_version(&pool, &restored, account, stored.key_version).await?;
    let decoded = receipt::open(&key, &context, &stored)?;
    assert_eq!(decoded.access_token.as_deref(), Some("access-secret"));
    assert_eq!(decoded.refresh_token.as_deref(), Some("refresh-secret"));
    let different_code = receipt::ReceiptContext {
        account,
        code_digest: rand::random(),
        purpose: "new".into(),
    };
    assert!(receipt::open(&key, &different_code, &stored).is_err());
    let wrong = receipt::ReceiptContext {
        account: other,
        ..context
    };
    assert!(receipt::open(&key, &wrong, &stored).is_err());
    pool.close().await;
    Ok(())
}
