//! Authentication context resolved from verified identities and live logins.

pub mod browser;
pub(crate) mod callbacks;
pub mod changes;
pub mod error;
pub(crate) mod oidc;
pub(crate) mod pending;
pub mod principal;
