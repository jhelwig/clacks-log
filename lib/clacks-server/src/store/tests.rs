//! Clacks account admission and ownership scenarios against production PostgreSQL.

use super::AccountStore;
use crate::auth::oidc::VerifiedIdentity;
use sqlx::postgres::PgPoolOptions;
use time::Duration;

#[tokio::test]
async fn concurrent_sign_ins_resolve_one_account() -> anyhow::Result<()> {
    let url = std::env::var("CLACKS_TEST_DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await?;
    crate::maintenance::migrate::run(&pool).await?;
    let store = AccountStore::new(pool);
    let before = sqlx::query_file!("tests/sql/count_accounts.sql")
        .fetch_one(&store.pool)
        .await?
        .count;
    let subject = ulid::Ulid::generate().to_string();
    let identity = || VerifiedIdentity {
        issuer: "https://test-provider.example".into(),
        subject: subject.clone(),
        display_name: Some("Operator".into()),
    };
    let (left, right) = tokio::join!(
        store.provision_login(identity(), Duration::days(14)),
        store.provision_login(identity(), Duration::days(14))
    );
    let left = left?;
    let right = right?;
    assert_eq!(left.principal, right.principal);
    let after = sqlx::query_file!("tests/sql/count_accounts.sql")
        .fetch_one(&store.pool)
        .await?
        .count;
    assert_eq!(after, before + 1);
    assert_ne!(left.reference.secret(), right.reference.secret());
    assert_eq!(
        store
            .authenticate(&left.reference, Duration::days(14))
            .await?,
        Some(left.principal.clone())
    );
    store.logout(&left.reference).await?;
    assert_eq!(
        store
            .authenticate(&left.reference, Duration::days(14))
            .await?,
        None
    );
    assert_eq!(
        store
            .authenticate(&right.reference, Duration::days(14))
            .await?,
        Some(right.principal)
    );
    Ok(())
}
