//! Project native contact fields and applicable confirmation claims to ADI values.
//!
//! UTC timestamps, radio values, station identities and names, reports, comments,
//! and supported equipment fields come from saved facts. Contacted names and
//! geography use the snapshot's selected sources. ADIF coordinates use the explicit
//! point or the calculated center of the selected grid area. Confirmation evidence
//! supplies received status independently of publication.

use super::{ExportFormat, ExportWarning, ReadinessField};
use clacks_client_store::read::export::ExportContact;
use clacks_model::{
    contact::BandFrequency,
    location::RecordedLocation,
    recognition::RecognitionCatalog,
    scalar::UtcSecond,
    service::{adif, destination::Service},
};
use qso_io::adif::{
    Band, DataType, DxccEntity, FieldSection, Mode, SecondarySubdivision, StandardField, Submode,
    adi::parser::{
        options::ParseOpts,
        strict::{Section, StrictParser},
    },
    primary_subdivisions::{alaska, canada, hawaii, united_states},
};
use std::collections::BTreeSet;

/// QRZ's application confirmation status, independent of upload or download.
pub(super) const QRZ_CONFIRMATION_STATUS: &str = "APP_QRZLOG_STATUS";

/// QRZ's retained first confirmation date.
pub(super) const QRZ_CONFIRMATION_DATE: &str = "APP_QRZLOG_QSLDATE";

/// Formats one contact and collects its required-field issues before applying reference mappings.
#[derive(Default)]
pub(super) struct ContactFields {
    /// Common output fields keyed by canonical ADI record tag.
    pub fields: adif::AdifFields,
    /// Named optional conversion and omission diagnostics from the contact.
    pub warnings: Vec<ExportWarning>,
    /// Required values unavailable for the requested format.
    pub issues: Vec<ReadinessField>,
}

/// Check the ADI repertoire shared by formatted identifiers and reference bindings.
pub(super) fn printable(value: &str) -> bool {
    !value.trim().is_empty() && value.bytes().all(|byte| matches!(byte, b' '..=b'~'))
}

/// Parse a standard field allowed in an ADI QSO record.
pub(super) fn standard_record_field(tag: &str) -> Option<StandardField> {
    let field: StandardField = tag.parse().ok()?;
    (field.section() == FieldSection::Record
        && !field.data_types().iter().any(|kind| {
            matches!(
                kind,
                DataType::IntlString | DataType::IntlMultilineString | DataType::IntlCharacter
            )
        }))
    .then_some(field)
}

/// Convert line boundaries to CRLF for multiline fields or spaces for single-line fields.
/// Discard characters outside the printable ADI repertoire.
fn adi_text(value: &str, multiline: bool) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' | '\n' => {
                if ch == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                result.push_str(if multiline { "\r\n" } else { " " });
            }
            ' '..='~' => result.push(ch),
            _ => {}
        }
    }
    result
}

/// Check a nonempty value using the standard field's ADI type and repertoire.
pub(super) fn standard_value(field: StandardField, value: &str) -> bool {
    if field.data_types() == [DataType::String] {
        return printable(value);
    }
    if value.trim().is_empty() {
        return false;
    }
    // qso-io exposes its type validation through the strict reader.
    let validation_record = format!("<{}:{}>{value}<EOR>", field.as_str(), value.len());
    let mut parser = StrictParser::new(validation_record.as_bytes(), ParseOpts::default());
    matches!(
        (&mut parser).into_iter().next(),
        Some(Ok(Section::Record(_)))
    )
}

/// Format the complete UTC timestamp when its day is representable as an ADIF Date.
fn adi_contact_time(value: UtcSecond) -> Option<(String, String)> {
    let (date, clock) = adif::contact_time(value).ok()?;
    standard_value(StandardField::QsoDate, &date).then_some((date, clock))
}

/// Check the recorded callsign against the format's character requirements.
fn callsign(value: &str, format: ExportFormat) -> bool {
    if format == ExportFormat::Ordinary {
        return printable(value);
    }
    value
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'/')
        && value.bytes().any(|byte| byte.is_ascii_alphanumeric())
}

/// Map the snapshot's common facts once even when a targeted profile expands the entry.
pub(super) fn contact_fields(
    contact: &ExportContact,
    format: ExportFormat,
    catalog: &RecognitionCatalog,
    today: jiff::civil::Date,
) -> ContactFields {
    let mut output = ContactFields::default();
    let qso = &contact.qso;
    let their_call = qso.contacted_station().callsign.as_str();
    let own_call = qso.own_station().callsign.as_str();
    let operator = qso.own_context().operator.as_ref();
    let operator_call = operator
        .and_then(|value| value.callsign().as_ref())
        .map(|value| value.as_str());
    for (tag, value, issue) in [
        ("CALL", their_call, ReadinessField::ContactedCallsign),
        (
            "STATION_CALLSIGN",
            own_call,
            ReadinessField::StationCallsign,
        ),
    ] {
        if callsign(value, format) {
            output.set(tag, value);
        } else {
            output.issues.push(issue);
        }
    }
    if let Some(value) = operator_call {
        if callsign(value, format) {
            output.set("OPERATOR", value);
        } else {
            output.warn(
                contact,
                "OPERATOR",
                "Omitted an invalid recorded operator callsign.",
            );
        }
    }
    if format == ExportFormat::Pota && (their_call == own_call || Some(their_call) == operator_call)
    {
        output.issues.push(ReadinessField::DistinctCallsign);
    }
    let start = qso.start();
    let future = format == ExportFormat::Pota
        && jiff::civil::date(1970, 1, 1)
            .at(0, 0, 0, 0)
            .checked_add(jiff::SignedDuration::from_secs(start.unix_seconds()))
            .is_ok_and(|instant| instant.date() > today);
    match adi_contact_time(start) {
        Some((date, time)) if !future => {
            output.set("QSO_DATE", date);
            output.set("TIME_ON", time);
        }
        _ => output.issues.push(ReadinessField::StartTime),
    }
    if let Some(end) = qso.end() {
        if let Some((date, time)) = adi_contact_time(end) {
            output.set("QSO_DATE_OFF", date);
            output.set("TIME_OFF", time);
        } else {
            output.warn(
                contact,
                "QSO_DATE_OFF / TIME_OFF",
                "Omitted a timestamp outside ADI's date range.",
            );
        }
    }

    output.radio(
        contact,
        qso.primary(),
        "FREQ",
        "BAND",
        format == ExportFormat::Pota,
    );
    if let Some(receive) = qso.receive_override() {
        output.radio(contact, receive, "FREQ_RX", "BAND_RX", false);
    }
    for (tag, value) in adif::mode(qso.mode(), catalog) {
        let supported = match tag.as_str() {
            "MODE" => value
                .parse::<Mode>()
                .is_ok_and(|mode| !mode.is_import_only()),
            "SUBMODE" => value.parse::<Submode>().is_ok(),
            _ => false,
        };
        if supported {
            output.fields.insert(tag, value);
        } else {
            output.warn(
                contact,
                &tag,
                "Omitted an unsupported recorded mode identifier.",
            );
        }
    }
    if format == ExportFormat::Pota
        && !output.fields.contains_key("MODE")
        && !output.fields.contains_key("SUBMODE")
    {
        output.issues.push(ReadinessField::Mode);
    }
    if let Some(power) = qso.own_context().transmit_power {
        output.set("TX_PWR", adif::power(power));
    }

    output.text(
        contact,
        StandardField::RstSent,
        qso.sent_report().as_deref(),
    );
    output.text(
        contact,
        StandardField::RstRcvd,
        qso.received_report().as_deref(),
    );
    if let Some(comment) = qso.comment() {
        let field = if comment.contains(['\r', '\n']) {
            StandardField::Notes
        } else {
            StandardField::Comment
        };
        output.text(contact, field, Some(comment));
    }
    if let Some(operator) = operator {
        output.text(
            contact,
            StandardField::MyName,
            operator.recorded_name().as_deref(),
        );
    }
    let recorded_contact_operator = qso.contacted_operator().as_ref();
    if let Some(operator) = recorded_contact_operator
        && let Some(value) = operator.callsign()
    {
        if callsign(value.as_str(), format) {
            output.set("CONTACTED_OP", value.as_str());
        } else {
            output.warn(
                contact,
                "CONTACTED_OP",
                "Omitted an invalid recorded operator callsign.",
            );
        }
    }
    let name = contact
        .facts
        .name
        .as_ref()
        .map(|value| value.value.as_str())
        .or_else(|| {
            recorded_contact_operator.and_then(|operator| operator.recorded_name().as_deref())
        });
    output.text(contact, StandardField::Name, name);
    if let Some(equipment) = &qso.own_context().equipment {
        output.text(
            contact,
            StandardField::MyRig,
            equipment.equipment_description().as_deref(),
        );
        output.text(
            contact,
            StandardField::MyAntenna,
            equipment.antenna_description().as_deref(),
        );
        if equipment.notes().is_some() {
            output.warn(
                contact,
                "Equipment notes",
                "Omitted because ADI has no corresponding equipment notes field.",
            );
        }
    }

    let own_location = contact
        .facts
        .own_location
        .as_ref()
        .map(|value| value.value.location.as_ref())
        .unwrap_or(qso.own_station().location.as_ref());
    let their_location = contact
        .facts
        .contact_location
        .as_ref()
        .map(|value| value.value.location.as_ref())
        .unwrap_or(qso.contacted_station().location.as_ref());
    if let Some(location) = own_location {
        output.location(contact, location, true, format == ExportFormat::Ordinary);
    }
    if let Some(location) = their_location {
        output.location(contact, location, false, true);
    }
    output.confirmations(contact);
    output
}

impl ContactFields {
    /// Assign one formatted value to its canonical tag.
    fn set(&mut self, tag: &'static str, value: impl Into<String>) {
        self.fields.insert(tag.into(), value.into());
    }

    /// Record a contact's local export warning.
    fn warn(&mut self, contact: &ExportContact, field: impl Into<String>, detail: &'static str) {
        self.warnings
            .push(ExportWarning::new(contact, field, detail));
    }

    /// Translate human-readable values using the particular field's ADI text type.
    fn text(&mut self, contact: &ExportContact, field: StandardField, value: Option<&str>) {
        let Some(value) = value else {
            return;
        };
        let tag = field.as_str();
        let converted = adif::text(tag, value);
        let multiline = field.data_types().contains(&DataType::MultilineString);
        let converted = converted.value.map(|value| adi_text(&value, multiline));
        match converted.filter(|value| !value.trim().is_empty()) {
            Some(result) => {
                if result != value {
                    self.warn(
                        contact,
                        tag,
                        "Text converted to the ADI character and line format.",
                    );
                }
                self.set(tag, result);
            }
            None => self.warn(
                contact,
                tag,
                "Omitted text with no representable ADI value.",
            ),
        }
    }

    /// Format recorded common transmit or receive values without deriving a band from frequency.
    fn radio(
        &mut self,
        contact: &ExportContact,
        pair: &BandFrequency,
        frequency: &'static str,
        band: &'static str,
        require_band: bool,
    ) {
        if let Some(value) = pair.frequency() {
            self.set(frequency, adif::frequency(value));
        }
        if let Some(value) = pair.band() {
            let value = value.label();
            if let Ok(code) = value.parse::<Band>() {
                self.set(band, code.as_str());
                return;
            }
            if require_band && value.eq_ignore_ascii_case("SUBMM") {
                self.set(band, "SUBMM");
                return;
            }
            self.warn(
                contact,
                band,
                "Omitted an unsupported recorded band identifier.",
            );
        }
        if require_band {
            self.issues.push(ReadinessField::TransmitBand);
        }
    }

    /// Map one whole station location and export the available point or grid center.
    fn location(
        &mut self,
        contact: &ExportContact,
        location: &RecordedLocation,
        own: bool,
        include_state: bool,
    ) {
        let (grid, extension, latitude, longitude, dxcc, state, county, cq, itu) = if own {
            (
                "MY_GRIDSQUARE",
                "MY_GRIDSQUARE_EXT",
                "MY_LAT",
                "MY_LON",
                "MY_DXCC",
                "MY_STATE",
                "MY_CNTY",
                "MY_CQ_ZONE",
                "MY_ITU_ZONE",
            )
        } else {
            (
                "GRIDSQUARE",
                "GRIDSQUARE_EXT",
                "LAT",
                "LON",
                "DXCC",
                "STATE",
                "CNTY",
                "CQZ",
                "ITUZ",
            )
        };
        if let Some(position) = location.position() {
            let locator = position.display_locator().to_string();
            if let Some((base, ext)) = locator.split_at_checked(8) {
                self.set(grid, base);
                if !ext.is_empty() {
                    self.set(extension, ext);
                }
            } else {
                self.set(grid, locator);
            }
            let (lat, lon) = adif::coordinates(position.display_coordinates());
            self.set(latitude, lat);
            self.set(longitude, lon);
        }
        let mut entity = None;
        if let Some(value) = location.dxcc() {
            if let Ok(recognized) = value.parse::<DxccEntity>() {
                entity = Some(recognized);
                self.set(dxcc, value);
            } else {
                self.warn(
                    contact,
                    dxcc,
                    "Omitted an unsupported DXCC entity identifier.",
                );
            }
        }
        if include_state {
            self.location_code(
                contact,
                state,
                location.primary_subdivision().as_deref(),
                |value| primary_subdivision(value, entity),
            );
        }
        self.location_code(
            contact,
            county,
            location.secondary_subdivision().as_deref(),
            // For some entities the catalog defines a code or county-name format
            // without enumerating each subdivision's membership.
            |value| SecondarySubdivision::candidates(value).next().is_some(),
        );
        if let Some(zone) = location.cq_zone() {
            self.set(cq, zone.get().to_string());
        }
        if let Some(zone) = location.itu_zone() {
            self.set(itu, zone.get().to_string());
        }
    }

    /// Retain a subdivision identifier exactly when it can be represented in ADI.
    fn location_code(
        &mut self,
        contact: &ExportContact,
        field: &'static str,
        value: Option<&str>,
        supported: impl FnOnce(&str) -> bool,
    ) {
        if let Some(value) = value {
            if printable(value) && supported(value) {
                self.set(field, value);
            } else {
                self.warn(
                    contact,
                    field,
                    "Omitted a subdivision code unsupported by the ADIF catalog.",
                );
            }
        }
    }

    /// Map applicable received status and retained dates with the same event meaning.
    fn confirmations(&mut self, contact: &ExportContact) {
        for (confirmed, service, status_field, status, date_field) in [
            (
                contact.facts.lotw.confirmed,
                Service::Lotw,
                "LOTW_QSL_RCVD",
                "Y",
                "LOTW_QSLRDATE",
            ),
            (
                contact.facts.qrz.confirmed,
                Service::Qrz,
                QRZ_CONFIRMATION_STATUS,
                "C",
                QRZ_CONFIRMATION_DATE,
            ),
        ] {
            if !confirmed {
                continue;
            }
            self.set(status_field, status);
            let mut dates = BTreeSet::new();
            let mut omitted = false;
            for claim in contact
                .confirmation_dates
                .iter()
                .filter(|claim| claim.service == service)
            {
                // LOG4OM receipt dates do not describe QRZ's first confirmation.
                if service == Service::Qrz && !qrz_first_confirmation(&claim.provenance.mapping) {
                    omitted = true;
                    continue;
                }
                if let Some((date, _)) = adi_contact_time(claim.date) {
                    dates.insert(date);
                } else {
                    omitted = true;
                }
            }
            if dates.len() == 1 {
                if let Some(date) = dates.into_iter().next() {
                    self.set(date_field, date);
                }
            } else if !dates.is_empty() {
                omitted = true;
            }
            if omitted {
                self.warn(contact, date_field, "Omitted a confirmation date with incompatible or conflicting retained evidence.");
            }
        }
    }
}

/// Identify the actual ADI field contributing the retained date in import provenance.
///
/// The import adapter appends field evidence after its source metadata. Match
/// the named field with its recorded byte location rather than a header value.
fn qrz_first_confirmation(mapping: &str) -> bool {
    mapping.rsplit(';').next().is_some_and(|fields| {
        fields.split(", ").any(|field| {
            field
                .trim_start()
                .strip_prefix(QRZ_CONFIRMATION_DATE)
                .is_some_and(|span| span.starts_with(" at bytes "))
        })
    })
}

/// Verify the primary code against its North American entity or the ADIF code catalog.
///
/// qso-io exposes country-specific enums and a strict field reader, but its
/// generic primary-subdivision check is internal. For other entities, reuse the
/// strict reader to check the catalog's field repertoire. That check does not
/// establish the geographic applicability of the code to a specific station.
fn primary_subdivision(value: &str, entity: Option<DxccEntity>) -> bool {
    match entity {
        Some(DxccEntity::UnitedStates) => {
            value.parse::<united_states::PrimarySubdivision>().is_ok()
        }
        Some(DxccEntity::Canada) => value.parse::<canada::PrimarySubdivision>().is_ok(),
        Some(DxccEntity::Alaska) => value.parse::<alaska::PrimarySubdivision>().is_ok(),
        Some(DxccEntity::Hawaii) => value.parse::<hawaii::PrimarySubdivision>().is_ok(),
        _ => standard_value(StandardField::State, value),
    }
}
