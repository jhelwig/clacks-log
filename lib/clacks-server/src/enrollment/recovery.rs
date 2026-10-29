//! Account-owned recovery selection without treating desktop hints as authority.

use super::ApprovalPurpose;
use crate::{
    auth::{error::AuthError, principal::Principal},
    runtime::ServerState,
};
use clacks_model::ids::{AccountId, ClientActorId};

/// Resolve current recovery eligibility only within the browser's verified account.
pub(super) async fn purpose(
    state: &ServerState,
    principal: &Principal,
    expected: Option<AccountId>,
    actor: Option<ClientActorId>,
) -> Result<ApprovalPurpose, AuthError> {
    match (expected, actor) {
        (None, None) => Ok(ApprovalPurpose::New),
        (Some(account), Some(actor)) if account == principal.account_id => {
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
