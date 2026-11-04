//! Checked native fixture edits shared by desktop behavior scenarios.

use clacks_model::contact::{Qso, input::QsoFields};

/// Apply a valid fixture correction through the saved-contact boundary.
pub(crate) fn edit(
    qso: &mut Qso,
    change: impl FnOnce(&mut QsoFields) -> Result<(), Box<dyn std::error::Error>>,
) {
    let mut fields = qso.clone().into_fields();
    change(&mut fields).expect("valid fixture input");
    *qso = Qso::try_from(fields).expect("valid contact fixture edit");
}

pub(crate) mod ws_suite;
