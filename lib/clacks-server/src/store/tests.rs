//! Clacks account admission and ownership scenarios against production PostgreSQL.

use super::AccountStore;
use crate::auth::oidc::VerifiedIdentity;
use jiff::SignedDuration;
use sqlx::postgres::PgPoolOptions;

#[tokio::test]
async fn concurrent_sign_ins_resolve_one_account() -> anyhow::Result<()> {
    let url = std::env::var("CLACKS_TEST_DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await?;
    crate::maintenance::migrate::run(&pool).await?;
    let store = AccountStore::new(pool);
    let subject = ulid::Ulid::generate().to_string();
    let identity = || VerifiedIdentity {
        issuer: "https://test-provider.example".into(),
        subject: subject.clone(),
        display_name: Some("Operator".into()),
    };
    let (left, right) = tokio::join!(
        store.provision_login(identity(), SignedDuration::from_hours(14 * 24)),
        store.provision_login(identity(), SignedDuration::from_hours(14 * 24))
    );
    let left = left?;
    let right = right?;
    let principal = store
        .authenticate(&left, SignedDuration::from_hours(14 * 24))
        .await?
        .unwrap();
    assert_eq!(
        store
            .authenticate(&right, SignedDuration::from_hours(14 * 24))
            .await?,
        Some(principal.clone())
    );
    assert_ne!(left.secret(), right.secret());
    assert_eq!(
        store
            .authenticate(&left, SignedDuration::from_hours(14 * 24))
            .await?,
        Some(principal.clone())
    );
    store.logout(&left).await?;
    assert_eq!(
        store
            .authenticate(&left, SignedDuration::from_hours(14 * 24))
            .await?,
        None
    );
    assert_eq!(
        store
            .authenticate(&right, SignedDuration::from_hours(14 * 24))
            .await?,
        Some(principal.clone())
    );
    let action = super::identities::IdentityAction {
        account_id: principal.account_id,
        originating_login: right.secret().to_owned(),
        authorizing_identity: principal.identity_id,
    };
    let second_subject = format!("{subject}-second");
    store
        .link_identity(
            &action,
            VerifiedIdentity {
                issuer: "https://test-provider.example".into(),
                subject: second_subject.clone(),
                display_name: None,
            },
        )
        .await?;
    let second = store
        .provision_login(
            VerifiedIdentity {
                issuer: "https://test-provider.example".into(),
                subject: second_subject.clone(),
                display_name: None,
            },
            SignedDuration::from_hours(14 * 24),
        )
        .await?;
    let second_principal = store
        .authenticate(&second, SignedDuration::from_hours(14 * 24))
        .await?
        .unwrap();
    let outsider = store
        .provision_login(
            VerifiedIdentity {
                issuer: "https://test-provider.example".into(),
                subject: format!("{subject}-outsider"),
                display_name: None,
            },
            SignedDuration::from_hours(14 * 24),
        )
        .await?;
    let outsider_principal = store
        .authenticate(&outsider, SignedDuration::from_hours(14 * 24))
        .await?
        .unwrap();
    let refused = store
        .link_identity(
            &action,
            VerifiedIdentity {
                issuer: "https://test-provider.example".into(),
                subject: format!("{subject}-outsider"),
                display_name: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        refused.downcast_ref::<super::identities::IdentityError>(),
        Some(super::identities::IdentityError::OwnedElsewhere)
    ));
    assert!(
        store
            .unlink_identity(&action, outsider_principal.identity_id)
            .await
            .is_err()
    );
    assert_eq!(store.identities(&outsider_principal).await?.len(), 1);
    let competing = super::identities::IdentityAction {
        account_id: principal.account_id,
        originating_login: second.secret().to_owned(),
        authorizing_identity: second_principal.identity_id,
    };
    let (remove_second, remove_first) = tokio::join!(
        store.unlink_identity(&action, second_principal.identity_id),
        store.unlink_identity(&competing, principal.identity_id)
    );
    assert_ne!(remove_second.is_ok(), remove_first.is_ok());
    let first_live = store
        .authenticate(&right, SignedDuration::from_hours(14 * 24))
        .await?;
    let second_live = store
        .authenticate(&second, SignedDuration::from_hours(14 * 24))
        .await?;
    assert_ne!(first_live.is_some(), second_live.is_some());
    let survivor = first_live.clone().or(second_live.clone()).unwrap();
    assert_eq!(store.identities(&survivor).await?.len(), 1);
    store
        .set_administrator(outsider_principal.account_id, true)
        .await?;
    assert!(store.is_administrator(&outsider_principal).await?);
    store
        .set_administrator(outsider_principal.account_id, false)
        .await?;
    assert!(!store.is_administrator(&outsider_principal).await?);
    store
        .recover_identity(
            survivor.account_id,
            VerifiedIdentity {
                issuer: "https://test-provider.example".into(),
                subject: format!("{subject}-recovery"),
                display_name: None,
            },
        )
        .await?;
    assert_eq!(
        store
            .authenticate(&right, SignedDuration::from_hours(14 * 24))
            .await?,
        first_live
    );
    assert_eq!(
        store
            .authenticate(&second, SignedDuration::from_hours(14 * 24))
            .await?,
        second_live
    );
    let recovered = store
        .provision_login(
            VerifiedIdentity {
                issuer: "https://test-provider.example".into(),
                subject: format!("{subject}-recovery"),
                display_name: None,
            },
            SignedDuration::from_hours(14 * 24),
        )
        .await?;
    let recovered_principal = store
        .authenticate(&recovered, SignedDuration::from_hours(14 * 24))
        .await?
        .unwrap();
    assert_eq!(recovered_principal.account_id, survivor.account_id);
    assert!(
        store
            .recover_identity(
                survivor.account_id,
                VerifiedIdentity {
                    issuer: "https://test-provider.example".into(),
                    subject: format!("{subject}-outsider"),
                    display_name: None,
                }
            )
            .await
            .is_err()
    );
    Ok(())
}

pub(crate) mod enrollment;

mod support;

mod directory;
mod key_rotation;

mod migration;
mod synchronization;
