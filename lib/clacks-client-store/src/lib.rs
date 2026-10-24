//! Durable relational storage for one local account workspace.

pub mod error;
pub mod identity;
pub mod operating;

mod metadata;
mod migration;
mod payload;
mod rows;
pub mod save;
pub mod store;
