//! Native recorded values and pure model operations for Clacks Log.
//!
//! Contacts carry station snapshots, copied equipment and complete logbook-tag
//! sets. Recognition catalogs, reference definitions and provider observations
//! supply interpretation without silently rewriting recorded contact facts.
//!
//! # Accepting records
//!
//! Scalar constructors enforce their local invariants. Composite records such as
//! [`Qso`] have public fields and require explicit preparation and validation:
//! normalize optional entry values, then call [`Qso::validate`] before saving.
//! Account ownership, reference existence, durable transactions and causal
//! synchronization remain application responsibilities.
//!
//! # Interpreting and exporting
//!
//! Use [`Mode::interpret`] and [`Band::interpret`] for current catalog views, and
//! [`select_location`] or [`select_name`] for eligible-source fallback. These do
//! not populate recorded fields. [`plan_reference_export`] produces in-memory
//! reference rows and required decisions, not files or a complete submission.
//!
//! # Entity identity
//!
//! Entity IDs are distinct types so unrelated record identities cannot be mixed:
//! ```compile_fail
//! use clacks_model::{QsoId, TagId};
//! fn accepts_contact(_: QsoId) {}
//! accepts_contact(TagId::new());
//! ```

#![deny(
    missing_docs,
    clippy::missing_docs_in_private_items,
    clippy::indexing_slicing
)]

mod error;
mod ids;
mod scalar;

pub use error::*;
pub use ids::*;
pub use scalar::*;

mod location;
mod position;

pub use location::*;
pub use position::*;

mod recognition;

pub use recognition::*;

mod contact;
mod equipment;
mod reference;

pub use contact::*;
pub use equipment::*;
pub use reference::*;

mod profile;
mod tag;

pub use profile::*;
pub use tag::*;

mod catalog;
mod export;
mod group;

pub use catalog::*;
pub use export::*;
pub use group::*;

mod effective;

pub use effective::*;
