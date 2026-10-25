//! Durable relational storage for one local account workspace.

pub mod error;
pub mod identity;
pub mod operating;
pub mod read;
pub mod retention;
pub mod save;
pub mod settings;
pub mod store;
pub mod synchronization;
pub mod transfer;

mod metadata;
mod migration;
mod payload;
mod rows;

#[cfg(test)]
mod failure_tests;
