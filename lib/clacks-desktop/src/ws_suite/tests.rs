//! Authored wire contracts, native projections and bounded pairing behavior.
mod contact;
mod pairing;
mod protocol;

fn context() -> anyhow::Result<super::contact::CaptureContext> {
    Ok(super::contact::CaptureContext {
        operating: clacks_client_store::operating::OperatingState::new(
            Some(clacks_model::profile::CurrentOperatingValues {
                station: clacks_model::contact::StationSnapshot {
                    callsign: clacks_model::scalar::RecordedCallsign::new("W7ZZZ")?,
                    location: None,
                    references: Default::default(),
                },
                operator: None,
                transmit_power: None,
                equipment: None,
            }),
            None,
            None,
        )?,
        selected_view: clacks_client_store::logbooks::selection::SelectedView::AllContacts,
        definitions: clacks_model::group::DefinitionSet::default(),
    })
}

mod runtime;
mod socket;
