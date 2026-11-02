//! Account-locked admission transitions coupled to OAuth grant activation.

use super::{ApprovalPurpose, ClientSummary, Principal};
use crate::oauth::{
    digest::{CredentialKind, digest},
    grant::GrantState,
    revocation,
    rows::{AccessRow, RefreshRow},
};
use crate::store::metadata::account::{load as account_in, save as save_account};
use clacks_model::ids::{AccountId, ClientActorId};
use clacks_sync::{
    account::AccountMetadata,
    membership::{ClientMembership, MembershipSnapshot},
    record::Position,
};
use oauth_as::{authorization::AuthorizationCodeRecord, scope::ScopeSet, store::RevocationWindow};
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;

/// Persisted client lifecycle fields shared by browser listings and membership snapshots.
struct MemberRow {
    /// Account owning this admission.
    account_id: String,
    /// Stable writer identity.
    actor_id: String,
    /// User-visible client label.
    label: String,
    /// Actual admission time.
    authorized_at: OffsetDateTime,
    /// Explicit closure time, if revoked.
    closed_at: Option<OffsetDateTime>,
    /// Timeline carrying the admission event.
    created_timeline: String,
    /// Revision carrying the admission event.
    created_revision: i64,
    /// Timeline carrying the closure event, if revoked.
    closed_timeline: Option<String>,
    /// Revision carrying the closure event, if revoked.
    closed_revision: Option<i64>,
    /// Actual durable receipt acknowledged by this client.
    acknowledged_checkpoint_id: Option<i64>,
}

/// Materialize a membership view from the same guarded connection as the account head.
pub(crate) async fn snapshot(
    connection: &mut PgConnection,
    account: &AccountMetadata,
) -> anyhow::Result<MembershipSnapshot> {
    let rows = sqlx::query_file_as!(
        MemberRow,
        "sql/enrollment/client_select.sql",
        account.account.to_string()
    )
    .fetch_all(&mut *connection)
    .await?;
    let mut clients = Vec::new();
    for row in rows {
        anyhow::ensure!(
            row.account_id == account.account.to_string(),
            "membership crossed account boundary"
        );
        let closed_at = match (row.closed_timeline, row.closed_revision) {
            (None, None) => None,
            (Some(timeline), Some(revision)) => Some(Position {
                timeline: timeline.parse()?,
                revision: revision.try_into()?,
            }),
            _ => anyhow::bail!("partial membership closure position"),
        };
        clients.push(ClientMembership {
            actor: row.actor_id.parse()?,
            created_at: Position {
                timeline: row.created_timeline.parse()?,
                revision: row.created_revision.try_into()?,
            },
            closed_at,
            acknowledged: match row.acknowledged_checkpoint_id {
                Some(id) => Some(
                    crate::store::metadata::lineage::load_checkpoint(
                        connection,
                        account.account,
                        id,
                    )
                    .await?,
                ),
                None => None,
            },
        });
    }
    let result = MembershipSnapshot {
        account: account.account,
        generation: account.membership_generation,
        clients,
    };
    result.validate(account)?;
    Ok(result)
}

/// Activate only the issued family identified by this code's successful conditional transition.
pub(super) async fn activate(
    connection: &mut PgConnection,
    code: &AuthorizationCodeRecord,
) -> anyhow::Result<Option<ClientActorId>> {
    let hash = digest(CredentialKind::Code, &code.code);
    let Some(approval) = sqlx::query_file!("sql/oauth/approval_select.sql", hash.as_slice())
        .fetch_optional(&mut *connection)
        .await?
    else {
        return Ok(None);
    };
    if approval.account_id != code.subject || approval.expires_at <= OffsetDateTime::now_utc() {
        return Ok(None);
    }
    let Some((Some(access), Some(refresh))) = code.state.minted() else {
        return Ok(None);
    };
    let access_hash = digest(CredentialKind::Access, access);
    let refresh_hash = digest(CredentialKind::Refresh, refresh);
    let Some(access) = sqlx::query_file_as!(
        AccessRow,
        "sql/oauth/access_select.sql",
        access_hash.as_slice()
    )
    .fetch_optional(&mut *connection)
    .await?
    else {
        return Ok(None);
    };
    let Some(refresh) = sqlx::query_file_as!(
        RefreshRow,
        "sql/oauth/refresh_select.sql",
        refresh_hash.as_slice()
    )
    .fetch_optional(&mut *connection)
    .await?
    else {
        return Ok(None);
    };
    let access_grant = GrantState::from_columns(
        &access.family_state,
        access.actor_id.as_deref(),
        access.provisional_until,
    )?;
    let refresh_grant = GrantState::from_columns(
        &refresh.family_state,
        refresh.actor_id.as_deref(),
        refresh.provisional_until,
    )?;
    let scope = ScopeSet::parse(&access.scope)?;
    if access.account_id != code.subject
        || refresh.account_id != code.subject
        || access.family_id != refresh.family_id
        || access.client_id != code.client_id.as_str()
        || refresh.client_id != access.client_id
        || access.key_thumbprint != refresh.key_thumbprint
        || Some(access.key_thumbprint.as_str()) != code.dpop_jkt.as_deref()
        || !matches!(access_grant, GrantState::Provisional { .. })
        || !matches!(refresh_grant, GrantState::Provisional { .. })
        || scope != ScopeSet::parse(&refresh.scope)?
        || !scope.is_subset(&code.scope)
        || access.resources != refresh.resources
        || !access
            .resources
            .iter()
            .all(|resource| code.resource.contains(resource))
    {
        return Ok(None);
    }
    let account_id: AccountId = code.subject.parse()?;
    let account = account_in(connection, account_id).await?;
    let members = snapshot(connection, &account).await?;
    let purpose =
        ApprovalPurpose::from_stored(&approval.purpose, approval.target_actor.as_deref())?;
    let actor = match purpose {
        ApprovalPurpose::New => new_actor(connection, &account, &members, &approval.label).await?,
        ApprovalPurpose::RecoverActive { actor } => {
            if !members
                .clients
                .iter()
                .any(|member| member.actor == actor && member.closed_at.is_none())
            {
                return Ok(None);
            }
            invalidate_families(connection, account_id, actor).await?;
            actor
        }
        ApprovalPurpose::ReplaceClosed { actor: target } => {
            if members
                .clients
                .iter()
                .any(|member| member.actor == target && member.closed_at.is_none())
            {
                return Ok(None);
            }
            new_actor(connection, &account, &members, &approval.label).await?
        }
    };
    let activated = sqlx::query_file!(
        "sql/enrollment/grant_activate.sql",
        access.family_id,
        actor.to_string()
    )
    .execute(connection)
    .await?
    .rows_affected();
    Ok((activated == 1).then_some(actor))
}

/// Plan and store one fresh actor without assigning a fictional installed checkpoint.
async fn new_actor(
    connection: &mut PgConnection,
    account: &AccountMetadata,
    members: &MembershipSnapshot,
    label: &str,
) -> anyhow::Result<ClientActorId> {
    let actor = ClientActorId::new();
    let update = clacks_sync::membership::enroll(actor, account, members)?;
    let revision = i64::try_from(update.next_account.head.revision)?;
    sqlx::query_file!(
        "sql/enrollment/client_insert.sql",
        account.account.to_string(),
        actor.to_string(),
        label,
        update.next_account.head.timeline.to_string(),
        revision
    )
    .execute(&mut *connection)
    .await?;
    save_account(connection, &update.next_account).await?;
    Ok(actor)
}

/// Invalidate replaced grant authority while retaining the client's admission history.
async fn invalidate_families(
    connection: &mut PgConnection,
    account: AccountId,
    actor: ClientActorId,
) -> anyhow::Result<()> {
    let families = sqlx::query_file!(
        "sql/enrollment/actor_families.sql",
        account.to_string(),
        actor.to_string()
    )
    .fetch_all(&mut *connection)
    .await?;
    for family in families {
        revocation::family_in(connection, &family.family_id, window()).await?;
    }
    Ok(())
}

/// Keep library revocation evidence through the maximum in-flight issuer horizon.
fn window() -> RevocationWindow {
    let now = std::time::SystemTime::now();
    RevocationWindow {
        recorded_at: now,
        until: now + std::time::Duration::from_secs(30 * 86400),
    }
}

/// Close actual membership and all actor-owned authorization in one account transaction.
pub(super) async fn revoke(
    pool: &PgPool,
    principal: &Principal,
    actor: ClientActorId,
) -> anyhow::Result<()> {
    let mut transaction = pool.begin().await?;
    let account_text = principal.account_id.to_string();
    let waiting = std::time::Instant::now();
    sqlx::query_file!("sql/account_lock.sql", account_text)
        .fetch_one(&mut *transaction)
        .await?;
    metrics::histogram!(crate::enrollment::ENROLLMENT_REVOCATION_LOCK_WAIT_SECONDS_METRIC)
        .record(waiting.elapsed().as_secs_f64());

    let valid = sqlx::query_file!(
        "sql/enrollment/principal.sql",
        account_text,
        principal.identity_id.to_string()
    )
    .fetch_one(&mut *transaction)
    .await?
    .valid;
    anyhow::ensure!(valid, "authorizing identity is no longer linked");
    let account = account_in(&mut transaction, principal.account_id).await?;
    let members = snapshot(&mut transaction, &account).await?;
    let update = clacks_sync::membership::revoke(actor, &account, &members)?;
    if update.next_account == account {
        return Ok(());
    }
    let revision = i64::try_from(update.next_account.head.revision)?;
    sqlx::query_file!(
        "sql/enrollment/client_close.sql",
        account_text,
        actor.to_string(),
        update.next_account.head.timeline.to_string(),
        revision
    )
    .execute(&mut *transaction)
    .await?;
    save_account(&mut transaction, &update.next_account).await?;
    invalidate_families(&mut transaction, principal.account_id, actor).await?;
    transaction.commit().await?;
    metrics::counter!("clacks_client_revocations_total").increment(1);
    tracing::info!(event="client.revoked", account_id=%principal.account_id, actor_id=%actor);
    Ok(())
}

/// Return account-owned labels and actual lifecycle times for browser management.
pub(super) async fn list(pool: &PgPool, account: AccountId) -> anyhow::Result<Vec<ClientSummary>> {
    let rows = sqlx::query_file_as!(
        MemberRow,
        "sql/enrollment/client_select.sql",
        account.to_string()
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ClientSummary {
                actor: row.actor_id.parse()?,
                label: row.label,
                authorized_at: row.authorized_at,
                closed_at: row.closed_at,
            })
        })
        .collect()
}
