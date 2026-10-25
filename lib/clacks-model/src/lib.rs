//! Native recorded values and pure model operations for Clacks Log.
//!
//! This library provides contact records, station snapshots, logbook tags,
//! reference definitions, recognition catalogs and pure domain operations.
//! It contains no persistence, networking or file-publication facilities.
//!
//! # API entry points
//!
//! - Contact acceptance: [`contact::Qso::normalize_optional_aggregates`] and [`contact::Qso::validate`].
//! - Recognition: [`recognition::mode::Mode::interpret`] and [`recognition::band::Band::interpret`].
//! - Effective information: [`effective::select_location`] and [`effective::select_name`].
//! - Reference output: [`export::evaluate::plan_reference_export`].
//!
//! # Entity identity
//!
//! Entity IDs are distinct types so unrelated record identities cannot be mixed:
//! ```compile_fail
//! use clacks_model::ids::{QsoId, TagId};
//! fn accepts_contact(_: QsoId) {}
//! accepts_contact(TagId::new());
//! ```

#![deny(
    missing_docs,
    clippy::missing_docs_in_private_items,
    clippy::indexing_slicing
)]
#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

pub mod defaults;
pub mod error;

pub mod ids;
pub mod scalar;

pub mod location;
pub mod position;

pub mod recognition;

pub mod contact;
pub mod equipment;
pub mod reference;

pub mod profile;
pub mod tag;

pub mod catalog;
pub mod export;
pub mod group;

pub mod effective;

pub mod service;
