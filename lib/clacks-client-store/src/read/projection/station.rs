//! Whole-location and name selection with the native source precedence.

use super::evidence::Evaluated;
use clacks_model::{
    contact::Qso,
    effective::{EligibleStationFacts, select_location, select_name},
    group::DefinitionSet,
};

/// Apply only eligible counterpart facts; returned own reports are not counterpart data.
pub(super) fn select(qso: &Qso, definitions: &DefinitionSet, result: &mut Evaluated) {
    let eligible = EligibleStationFacts {
        lotw: result.lotw.as_ref(),
        qrz: result.qrz.as_ref(),
        directory: None,
    };
    result.facts.name = select_name(
        qso.contacted_operator
            .as_ref()
            .and_then(|o| o.recorded_name.as_deref()),
        &eligible,
    );
    result.facts.contact_location = select_location(&qso.contacted_station, &eligible, definitions);
    result.facts.own_location = select_location(
        &qso.own_station,
        &EligibleStationFacts::default(),
        definitions,
    );
}
