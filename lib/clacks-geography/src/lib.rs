//! Immutable offline geographic bundles and whole-position geographic queries.

#![deny(
    missing_docs,
    clippy::missing_docs_in_private_items,
    clippy::indexing_slicing
)]
#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

pub mod artifact;
pub mod error;
pub mod geometry;
pub mod lookup;
