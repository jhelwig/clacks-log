//! Live WS facts projected into checked native values with captured local fallback.
use super::protocol::{CompletionFormat, DecodedCompletion, IncomingFacts, PrimaryIdentity};
use clacks_client_store::{
    logbooks::selection::SelectedView, operating::OperatingState, settings::SettingsSnapshot,
};
use clacks_geography::{artifact::Field, assistance::populate};
use clacks_model::{
    contact::{
        BandFrequency, OperatorInformationFields, OwnOperatingContext, Qso, StationSnapshot,
        input::QsoFields,
    },
    export::rules::{BindingCondition, ValueExpression},
    group::DefinitionSet,
    ids::QsoId,
    location::{CqZone, ItuZone, RecordedLocation, RecordedLocationFields},
    position::{Locator, Position},
    recognition::{
        RecognitionCatalog,
        band::Band,
        mode::{Mode, ModeIdentity},
    },
    reference::{ReferenceAssociation, StationSide},
    scalar::{Hertz, Milliwatts, RecordedCallsign},
};
use clacks_sync::record::{RecordState, RecordValue};
use std::collections::BTreeSet;

/// One immutable capture basis, independent of the unfinished manual contact.
#[derive(Clone, Debug)]
pub(crate) struct CaptureContext {
    /// Applied operating facts copied at candidate admission.
    pub operating: OperatingState,
    /// Selected tag or built-in view copied at admission.
    pub selected_view: SelectedView,
    /// Current account-owned group definitions from the same snapshot.
    pub definitions: DefinitionSet,
}

impl CaptureContext {
    /// Extract capture inputs from one ordinary bounded local snapshot.
    pub fn from_snapshot(snapshot: &SettingsSnapshot) -> anyhow::Result<Self> {
        let groups = snapshot
            .groups
            .iter()
            .filter_map(|record| match record.working_state() {
                Some(RecordState::Live(RecordValue::Group(group))) => Some(group.as_ref().clone()),
                _ => None,
            })
            .collect();
        Ok(Self {
            operating: snapshot.operating.clone(),
            selected_view: snapshot.logbooks.selected_view,
            definitions: DefinitionSet::new(groups)?,
        })
    }
}

/// Native QSO and field-level conversion warnings, without retaining raw messages.
#[derive(Clone, Debug)]
pub(crate) struct CaptureProjection {
    /// Complete checked contact, ready for the existing save operation.
    pub qso: Qso,
    /// Supplied own fields, including rejected nonempty supplied absence.
    pub own_protected: BTreeSet<Field>,
    /// Supplied contacted fields, including rejected nonempty supplied absence.
    pub contacted_protected: BTreeSet<Field>,
    /// Supplied optional fields omitted or unsupported in the native model.
    pub warnings: Vec<String>,
}

/// Convert decimal watts exactly, truncating digits below one milliwatt toward zero.
/// Nonempty invalid/zero/overflow values return an error and must not use defaults.
pub(crate) fn parse_power_watts(text: &str) -> anyhow::Result<Option<Milliwatts>> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    anyhow::ensure!(
        (!whole.is_empty() || !fraction.is_empty())
            && whole
                .bytes()
                .chain(fraction.bytes())
                .all(|c| c.is_ascii_digit()),
        "Invalid decimal watts."
    );
    let whole = if whole.is_empty() {
        0
    } else {
        whole.parse::<u64>()?
    };
    let fraction: String = fraction
        .chars()
        .take(3)
        .chain(std::iter::repeat('0'))
        .take(3)
        .collect();
    let value = whole
        .checked_mul(1000)
        .and_then(|value| {
            fraction
                .parse::<u64>()
                .ok()
                .and_then(|fraction| value.checked_add(fraction))
        })
        .ok_or_else(|| anyhow::anyhow!("Power out of range."))?;
    Ok(Some(Milliwatts::new(value)?))
}

/// Apply a supplied optional value, falling back only when the wire field is absent/empty.
fn optional<T>(
    facts: &IncomingFacts,
    field: &str,
    fallback: Option<T>,
    warnings: &mut Vec<String>,
    parse: impl FnOnce(&str) -> anyhow::Result<T>,
) -> Option<T> {
    let Some(text) = facts.text(field) else {
        return fallback;
    };
    match parse(text.trim()) {
        Ok(value) => Some(value),
        Err(_) => {
            warnings.push(field.into());
            None
        }
    }
}

/// Build recorded geography with field-by-field own fallback and no contacted defaults.
fn location(
    facts: &IncomingFacts,
    own: bool,
    fallback: Option<&RecordedLocation>,
    warnings: &mut Vec<String>,
) -> anyhow::Result<Option<RecordedLocation>> {
    let base = fallback
        .cloned()
        .map(RecordedLocation::into_fields)
        .unwrap_or_default();
    let names = if own {
        [
            "MY_GRIDSQUARE",
            "MY_DXCC",
            "MY_STATE",
            "MY_CNTY",
            "MY_CQ_ZONE",
            "MY_ITU_ZONE",
        ]
    } else {
        ["GRIDSQUARE", "DXCC", "STATE", "CNTY", "CQZ", "ITUZ"]
    };
    let [grid, dxcc, state, county, cq, itu] = names;
    let fields = RecordedLocationFields {
        position: optional(facts, grid, base.position, warnings, |text| {
            Ok(Position::GridArea(Locator::parse(text)?))
        }),
        dxcc: optional(facts, dxcc, base.dxcc, warnings, |text| Ok(text.into())),
        primary_subdivision: optional(facts, state, base.primary_subdivision, warnings, |text| {
            Ok(text.into())
        }),
        secondary_subdivision: optional(
            facts,
            county,
            base.secondary_subdivision,
            warnings,
            |text| Ok(text.into()),
        ),
        cq_zone: optional(facts, cq, base.cq_zone, warnings, |text| {
            Ok(CqZone::new(text.parse()?)?)
        }),
        itu_zone: optional(facts, itu, base.itu_zone, warnings, |text| {
            Ok(ItuZone::new(text.parse()?)?)
        }),
    };
    fields.normalized().map(TryInto::try_into).transpose()
}

/// Resolve a source-supplied local contest through eligible ordinary literal bindings.
pub(crate) fn contest(
    code: &str,
    definitions: &DefinitionSet,
) -> anyhow::Result<ReferenceAssociation> {
    let mut groups=definitions.iter().filter(|group|!group.identifier().required&&group.export_mappings().ordinary_interchange.iter().any(|binding|{
        binding.field.eq_ignore_ascii_case("CONTEST_ID")&&matches!(&binding.value,ValueExpression::Literal(value) if value.trim().eq_ignore_ascii_case(code.trim()))&&matches!(binding.when,BindingCondition::Always|BindingCondition::AssociationPresent(StationSide::Own))
    }));
    let group = groups
        .next()
        .ok_or_else(|| anyhow::anyhow!("Unmapped contest."))?;
    anyhow::ensure!(groups.next().is_none(), "Ambiguous contest.");
    ReferenceAssociation::new(group.id(), None)
}

/// Convert the chosen whole representation; invalid optional values are omitted with warnings.
/// Missing required facts reject only this representation, leaving its counterpart eligible.
#[tracing::instrument(skip_all, level="debug", fields(qso_id=%id, format=?notice.format))]
pub(crate) fn build(
    id: QsoId,
    notice: &DecodedCompletion,
    context: &CaptureContext,
) -> anyhow::Result<CaptureProjection> {
    let facts = &notice.facts;
    let active = context.operating.current();
    let mut warnings = facts.warnings.clone();
    let call = match facts.text("STATION_CALLSIGN") {
        Some(call) => RecordedCallsign::new(call.trim())?,
        None => active
            .map(|current| current.station.callsign.clone())
            .ok_or_else(|| anyhow::anyhow!("Our station callsign is missing."))?,
    };
    let own_protected = supplied_protection(facts, true);
    let contacted_protected = supplied_protection(facts, false);
    let own_location = location(
        facts,
        true,
        active.and_then(|current| current.station.location.as_ref()),
        &mut warnings,
    )?;
    let mut references = active
        .map(|current| current.station.references.clone())
        .unwrap_or_default();
    if let Some(code) = facts.text("CONTEST_ID") {
        match contest(code, &context.definitions) {
            Ok(reference) => {
                references.insert(reference);
            }
            Err(_) => warnings.push("CONTEST_ID".into()),
        }
    }
    let own_station = StationSnapshot {
        callsign: call,
        location: own_location,
        references,
    };
    let contacted_station = StationSnapshot {
        callsign: RecordedCallsign::new(&notice.identity.contacted_callsign)?,
        location: location(facts, false, None, &mut warnings)?,
        references: Default::default(),
    };
    let catalog = RecognitionCatalog::adif_318()?;
    let primary = match &notice.identity.primary {
        PrimaryIdentity::Frequency(hertz) => {
            BandFrequency::new(Some(Hertz::new(*hertz)?), None, catalog)?
        }
        PrimaryIdentity::Band(label) => {
            BandFrequency::new(None, Some(Band::from_recorded_label(label)?), catalog)?
        }
    };
    let mode = if let Some(submode) = facts.text("SUBMODE") {
        Mode::from_recorded_identity(ModeIdentity::Specialization {
            label: submode.trim().into(),
            parent: facts.text("MODE").map(|text| text.trim().into()),
        })?
    } else if notice.format == CompletionFormat::Structured {
        crate::logging::form::parse_mode(&notice.identity.mode, "")?
    } else {
        Mode::from_recorded_identity(ModeIdentity::Parent {
            label: notice.identity.mode.clone(),
        })?
    };
    let mut end = facts.end;
    if end.is_some_and(|end| end < facts.start) {
        end = None;
        warnings.push("TIME_OFF".into());
    }
    let own_operator = active.and_then(|current| current.operator.as_ref());
    let operator = OperatorInformationFields {
        callsign: optional(
            facts,
            "OPERATOR",
            own_operator.and_then(|operator| operator.callsign().clone()),
            &mut warnings,
            |text| Ok(RecordedCallsign::new(text)?),
        ),
        recorded_name: own_operator.and_then(|operator| operator.recorded_name().clone()),
    }
    .normalized()
    .map(TryInto::try_into)
    .transpose()?;
    let power = if let Some(text) = facts.text("TX_PWR") {
        match parse_power_watts(text) {
            Ok(power) => power,
            Err(_) => {
                warnings.push("TX_PWR".into());
                None
            }
        }
    } else {
        active.and_then(|current| current.transmit_power)
    };
    let receive_override = optional(facts, "FREQ_RX", None, &mut warnings, |text| {
        BandFrequency::new(crate::logging::radio::parse_mhz(text)?, None, catalog)
    });
    for field in [
        "PROP_MODE",
        "SAT_NAME",
        "SAT_MODE",
        "STX",
        "SRX",
        "STX_STRING",
        "SRX_STRING",
        "CLASS",
        "ARRL_SECT",
    ] {
        if facts.text(field).is_some() {
            warnings.push(field.into());
        }
    }
    let tags = match context.selected_view {
        SelectedView::Logbook(tag) => std::iter::once(tag).collect(),
        _ => Default::default(),
    };
    let contacted_operator = OperatorInformationFields {
        callsign: None,
        recorded_name: facts.text("NAME").map(str::to_owned),
    }
    .normalized()
    .map(TryInto::try_into)
    .transpose()?;
    let qso = QsoFields {
        id,
        start: facts.start,
        end,
        own_station,
        contacted_station,
        own_context: OwnOperatingContext {
            operator,
            transmit_power: power,
            equipment: active.and_then(|current| current.equipment.clone()),
        },
        contacted_operator,
        primary,
        receive_override,
        mode,
        sent_report: facts.text("RST_SENT").map(str::to_owned),
        received_report: facts.text("RST_RCVD").map(str::to_owned),
        comment: facts.text("COMMENT").map(str::to_owned),
        tags,
    }
    .try_into()?;
    Ok(CaptureProjection {
        qso,
        warnings,
        own_protected,
        contacted_protected,
    })
}

/// Mark wire eligibility before parsing; invalid nonempty input is protected absence.
fn supplied_protection(facts: &IncomingFacts, own: bool) -> BTreeSet<Field> {
    [
        (Field::Dxcc, if own { "MY_DXCC" } else { "DXCC" }),
        (
            Field::PrimarySubdivision,
            if own { "MY_STATE" } else { "STATE" },
        ),
        (
            Field::SecondarySubdivision,
            if own { "MY_CNTY" } else { "CNTY" },
        ),
        (Field::CqZone, if own { "MY_CQ_ZONE" } else { "CQZ" }),
        (Field::ItuZone, if own { "MY_ITU_ZONE" } else { "ITUZ" }),
    ]
    .into_iter()
    .filter_map(|(field, name)| facts.text(name).map(|_| field))
    .collect()
}

/// Assist the selected stable projection before a contact transaction is opened.
/// Failure and ambiguity leave a valid capture usable with its existing warnings.
pub(crate) async fn assist(
    mut projection: CaptureProjection,
    geography: &crate::geography::Client,
) -> CaptureProjection {
    let mut fields = projection.qso.clone().into_fields();
    assist_station(
        &mut fields.own_station,
        &projection.own_protected,
        geography,
    )
    .await;
    assist_station(
        &mut fields.contacted_station,
        &projection.contacted_protected,
        geography,
    )
    .await;
    match fields.try_into() {
        Ok(qso) => projection.qso = qso,
        Err(_) => tracing::error!(event = "geography.capture_projection_failed"),
    }
    projection
}

/// Populate eligible native station fields, retaining all non-geographic station facts.
async fn assist_station(
    station: &mut StationSnapshot,
    protected: &BTreeSet<Field>,
    geography: &crate::geography::Client,
) {
    let Some(location) = station.location.as_ref() else {
        return;
    };
    let Some(position) = location.position().as_ref().cloned() else {
        return;
    };
    let Ok(result) = geography.lookup(position).await else {
        return;
    };
    let mut fields = location.clone().into_fields();
    populate(&mut fields, protected, &result);
    match fields.try_into() {
        Ok(location) => station.location = Some(location),
        Err(_) => tracing::error!(event = "geography.capture_station_failed"),
    }
}
