//! Authentication context resolved from verified identities and live logins.

pub(crate) mod oidc;
pub mod principal;

pub(crate) mod callbacks;
pub mod error;
pub(crate) mod pending;

pub(crate) mod changes;
