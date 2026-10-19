use crate::{DefinitionSet, RecordedLocation, StationSnapshot, optional_text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InformationSource {
    Recorded,
    Lotw,
    Qrz,
    Directory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sourced<T> {
    pub value: T,
    pub source: InformationSource,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StationFacts {
    pub name: Option<String>,
    pub location: Option<RecordedLocation>,
}

/// Read projection only. The caller excludes stale-key and otherwise ineligible observations.
#[derive(Clone, Copy, Debug, Default)]
pub struct EligibleStationFacts<'a> {
    pub lotw: Option<&'a StationFacts>,
    pub qrz: Option<&'a StationFacts>,
    pub directory: Option<&'a StationFacts>,
}

impl<'a> EligibleStationFacts<'a> {
    fn sources(&self) -> [(InformationSource, Option<&'a StationFacts>); 3] {
        [
            (InformationSource::Lotw, self.lotw),
            (InformationSource::Qrz, self.qrz),
            (InformationSource::Directory, self.directory),
        ]
    }
}

pub fn select_location(
    recorded: &StationSnapshot,
    eligible: &EligibleStationFacts<'_>,
    definitions: &DefinitionSet,
) -> Option<Sourced<Option<RecordedLocation>>> {
    let location = recorded
        .location
        .as_ref()
        .filter(|l| l.has_values())
        .cloned();
    if location.is_some()
        || recorded.references.iter().any(|r| {
            definitions
                .get(r.group_id)
                .is_some_and(|d| d.bears_location_context)
        })
    {
        return Some(Sourced {
            value: location,
            source: InformationSource::Recorded,
        });
    }
    eligible.sources().into_iter().find_map(|(source, facts)| {
        facts
            .and_then(|f| f.location.as_ref())
            .filter(|l| l.has_values())
            .map(|l| Sourced {
                value: Some(l.clone()),
                source,
            })
    })
}

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

/// Both inputs are already-normalized keys supplied by directory adapters.
pub fn directory_key_matches(current: &str, snapshot_key: &str) -> bool {
    current == snapshot_key
}
