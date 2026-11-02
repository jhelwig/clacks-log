//! Stable workspace-local name of the protected directory value.

use crate::{local_state, store::ClientStore};

/// Local locator only; configuration and credentials are stored together natively.
const KEY: &str = "directory.entry";

/// Locate this workspace's protected directory entry, allocating its fixed name once.
///
/// Persist the name before native access so a failed write can retry at the same
/// location. Reauthorization and credential updates leave the name unchanged;
/// different workspaces remain independent even when enrolled in the same account.
pub async fn reference(store: &ClientStore) -> anyhow::Result<String> {
    let mut transaction = store.write().await?;
    let reference = match local_state::get_in(&mut transaction, KEY).await? {
        Some(reference) if !reference.is_empty() => reference,
        _ => {
            let reference = ulid::Ulid::generate().to_string();
            local_state::set_in(&mut transaction, KEY, &reference).await?;
            reference
        }
    };
    transaction.commit().await?;
    Ok(reference)
}
