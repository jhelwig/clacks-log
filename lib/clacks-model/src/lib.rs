//! Native recorded values and pure model operations.
//!
//! Entity IDs are deliberately distinct types:
//! ```compile_fail
//! use clacks_model::{QsoId, TagId};
//! fn accepts_contact(_: QsoId) {}
//! accepts_contact(TagId::new());
//! ```

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
