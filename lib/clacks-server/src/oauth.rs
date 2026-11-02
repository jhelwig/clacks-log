//! Embedded OAuth issuer and direct account/client authorization.

/// Metric name for expired issuer rows removed by confirmed commits.
pub(crate) const OAUTH_EXPIRED_ROWS_TOTAL_METRIC: &str = "clacks_oauth_expired_rows_total";

pub(crate) mod authorization;
pub(crate) mod cleanup;
pub(crate) mod clients;
pub(crate) mod config;
pub(crate) mod digest;
pub(crate) mod error;
mod events;
pub(crate) mod grant;
pub(crate) mod http;
pub(crate) mod replay;
pub(crate) mod resource;
pub(crate) mod revocation;
pub(crate) mod rows;
pub(crate) mod store;
pub(crate) mod tokens;
