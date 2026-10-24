//! Durable relational storage for one local account workspace.

pub mod error;
pub mod identity;
mod migration;
mod payload;
mod rows;
pub mod store;
