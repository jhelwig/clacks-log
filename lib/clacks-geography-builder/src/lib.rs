//! Pinned-source geographic data preparation and immutable publication.

#![deny(
    missing_docs,
    clippy::missing_docs_in_private_items,
    clippy::indexing_slicing
)]
#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

pub mod application;
pub mod recipe;
pub mod source;
