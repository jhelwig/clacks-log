//! The account and login identity authorized for a request.

use clacks_model::ids::AccountId;
use ulid::Ulid;

/// Live authentication context; never contains provider or session secrets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Principal {
    /// Stable internal account owning this request's data.
    pub account_id: AccountId,
    /// Linked identity used to establish this login.
    pub identity_id: Ulid,
}
