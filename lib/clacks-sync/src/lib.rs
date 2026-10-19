//! Production synchronization metadata and conditional state transitions.
//!
//! Returned updates require real storage transactions; they do not certify persistence.

#![deny(
    missing_docs,
    clippy::missing_docs_in_private_items,
    clippy::indexing_slicing
)]
#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

pub mod clock;
pub mod error;
pub mod record;

pub mod account;
pub mod lineage;

pub mod compare;

pub mod client;

pub mod acceptance;
pub mod membership;

pub mod transfer;

pub mod compaction;
