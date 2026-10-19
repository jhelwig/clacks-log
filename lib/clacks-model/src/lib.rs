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
