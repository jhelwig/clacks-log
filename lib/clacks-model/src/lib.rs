//! Native recorded values and pure model operations for Clacks Log.
//!
//! This library provides contact records, station snapshots, logbook tags,
//! reference definitions, recognition catalogs and pure domain operations.
//! It contains no persistence, networking or file-publication facilities.
//!
//! # API entry points
//!
//! - Contact acceptance: [`Qso::normalize_optional_aggregates`] and [`Qso::validate`].
//! - Recognition: [`Mode::interpret`] and [`Band::interpret`].
//! - Effective information: [`select_location`] and [`select_name`].
//! - Reference output: [`plan_reference_export`].
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
