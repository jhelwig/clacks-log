//! Read-only recorded/provider fallback with explicit provenance and eligibility boundaries.

use crate::contact::StationSnapshot;
use crate::group::DefinitionSet;
use crate::location::RecordedLocation;
use crate::scalar::optional_text;
/// Origin selected for an effective display/export value; not permission to edit recorded facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InformationSource {
    /// Facts explicitly recorded with the contact or its location-bearing references.
    Recorded,
    /// Eligible Logbook of The World (LoTW) observation.
    Lotw,
    /// Eligible QRZ observation.
    Qrz,
    /// Eligible snapshot from the account-selected callsign directory.
    Directory,
}

/// An effective value paired with its selected information source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sourced<T> {
    /// Selected value, including an authoritative absence when the payload permits it.
    pub value: T,
    /// Origin governing the effective value.
    pub source: InformationSource,
}

/// Provider-supplied name and location facts kept separate from recorded station information.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StationFacts {
    /// Provider-supplied name, not a manually recorded QSO field.
    pub name: Option<String>,
    /// Whole provider location; absent details are not patched from other providers.
    pub location: Option<RecordedLocation>,
}

/// Provider observations that the caller has already established are eligible.
///
/// Exclude stale directory keys, obsolete versions and other ineligible data before
/// calling selectors. This projection does not perform provider matching or
/// choose between concurrent snapshots.
#[derive(Clone, Copy, Debug, Default)]
pub struct EligibleStationFacts<'a> {
    /// Already-eligible Logbook of The World (LoTW) station facts.
    pub lotw: Option<&'a StationFacts>,
    /// Already-eligible QRZ station facts.
    pub qrz: Option<&'a StationFacts>,
    /// Already-eligible selected-directory snapshot matching the current lookup key.
    pub directory: Option<&'a StationFacts>,
}

impl<'a> EligibleStationFacts<'a> {
    /// Enumerate eligible fallback sources in LoTW, QRZ and directory priority order.
    fn sources(&self) -> [(InformationSource, Option<&'a StationFacts>); 3] {
        [
            (InformationSource::Lotw, self.lotw),
            (InformationSource::Qrz, self.qrz),
            (InformationSource::Directory, self.directory),
        ]
    }
}

/// Choose the first eligible whole location: recorded, LoTW, QRZ, then directory.
///
/// Recorded location-bearing participation suppresses provider fallback even
/// when its geography is unknown: `Some` with `value: None` represents that
/// authoritative unknown location. Outer `None` means no source supplies one.
/// Fields from different sources are never mixed and inputs are never changed.
pub fn select_location(
    recorded: &StationSnapshot,
    eligible: &EligibleStationFacts<'_>,
    definitions: &DefinitionSet,
) -> Option<Sourced<Option<RecordedLocation>>> {
    let location = recorded.location.as_ref().cloned();
    if location.is_some()
        || recorded.references.iter().any(|r| {
            definitions
                .get(r.group_id)
                .is_some_and(|d| d.bears_location_context())
        })
    {
        return Some(Sourced {
            value: location,
            source: InformationSource::Recorded,
        });
    }
    eligible.sources().into_iter().find_map(|(source, facts)| {
        facts.and_then(|f| f.location.as_ref()).map(|l| Sourced {
            value: Some(l.clone()),
            source,
        })
    })
}

/// Select nonblank recorded name text, then eligible LoTW, QRZ or directory text.
///
/// Use for logical display/export fallback. Blank values are skipped, nonblank
/// text is preserved, and the recorded contact is never populated or overwritten.
pub fn select_name(
    recorded: Option<&str>,
    eligible: &EligibleStationFacts<'_>,
) -> Option<Sourced<String>> {
    if let Some(name) = recorded.and_then(optional_text) {
        return Some(Sourced {
            value: name,
            source: InformationSource::Recorded,
        });
    }
    eligible.sources().into_iter().find_map(|(source, facts)| {
        facts
            .and_then(|f| f.name.as_deref())
            .and_then(optional_text)
            .map(|name| Sourced {
                value: name,
                source,
            })
    })
}
