//! Account-owned recovery selection without treating desktop hints as authority.

use super::{ApprovalPurpose, RecoveryTarget};
use crate::{
    auth::{error::AuthError, principal::Principal},
    runtime::ServerState,
};

/// Resolve current recovery eligibility only within the browser's verified account.
pub(super) async fn purpose(
    state: &ServerState,
    principal: &Principal,
    target: Option<RecoveryTarget>,
) -> Result<ApprovalPurpose, AuthError> {
    match target {
        None => Ok(ApprovalPurpose::New),
        Some(RecoveryTarget { account, actor }) if account == principal.account_id => {
            let clients = super::clients(state, principal)
                .await
                .map_err(AuthError::from_store)?;
            if clients
                .iter()
                .any(|client| client.actor == actor && client.closed_at.is_none())
            {
                Ok(ApprovalPurpose::RecoverActive { actor })
            } else {
                Ok(ApprovalPurpose::ReplaceClosed { actor })
            }
        }
        _ => Err(AuthError::Conflict(
            "Sign in to the account associated with this desktop workspace.".into(),
        )),
    }
}
