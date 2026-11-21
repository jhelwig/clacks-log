//! Receiver-specific readiness and activation locations for POTA-compatible ADI.
//!
//! The installed targeted rules perform partitioning and row expansion. The
//! receiver uses each park's unqualified identity, an applicable subdivision
//! code in MY_STATE, and a single activation location per file.

use super::{ExportWarning, ReadinessField};
use clacks_client_store::read::export::ExportContact;
use clacks_model::{ids::GroupId, reference::ReferenceAssociation, service::adif::AdifFields};
use qso_io::adif::{
    DxccEntity,
    primary_subdivisions::{australia, canada, germany, mexico, united_states},
};
use std::collections::{BTreeMap, BTreeSet};

mod japan;

#[cfg(test)]
mod tests;

/// Activation selected for one declared own park reference.
pub(super) struct Activation<'a> {
    /// Unqualified park identity emitted in MY_SIG_INFO.
    pub park: &'a str,
    /// Partition's qualified location, empty when the receiver can prompt.
    pub location: String,
    /// Applicable subdivision code for the receiver.
    pub subdivision: Option<&'a str>,
    /// Whether the recorded primary subdivision was unusable for POTA output.
    unverified_subdivision: bool,
}

/// POTA-specific requirements and facts to use with the shared reference plan.
pub(super) struct ContactReadiness<'a> {
    /// Prepared activations addressed by the planner's exact raw own reference key.
    pub activations: BTreeMap<&'a str, Activation<'a>>,
    /// Optional contacted references after excluding unsupported park identifiers.
    pub contacted_references: BTreeSet<ReferenceAssociation>,
    /// Distinct unqualified contacted parks expected in each activation output.
    pub contacted_parks: BTreeSet<&'a str>,
}

/// Split a recorded reference into the exact park base and optional qualifier.
fn park_reference(value: &str) -> Option<(&str, Option<&str>)> {
    let (park, location) = value
        .split_once('@')
        .map_or((value, None), |(park, location)| (park, Some(location)));
    let (prefix, number) = park.rsplit_once('-')?;
    (!prefix.is_empty()
        && prefix.len() <= 4
        && prefix
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        && matches!(number.len(), 4 | 5)
        && number.bytes().all(|byte| byte.is_ascii_digit()))
    .then_some((park, location))
}

/// A country-qualified location supplied by a park qualifier or station record.
#[derive(Clone, Copy)]
struct Qualification<'a> {
    /// The alphabetic country prefix of the location code.
    country: &'a str,
    /// The subdivision suffix from the location code.
    subdivision: &'a str,
}

impl<'a> Qualification<'a> {
    /// Interpret the country and subdivision parts of a recorded location code.
    fn parse(value: &'a str) -> Option<Self> {
        let (country, subdivision) = value.split_once('-')?;
        (country.len() == 2
            && country.bytes().all(|byte| byte.is_ascii_uppercase())
            && subdivision_code(subdivision))
        .then_some(Self {
            country,
            subdivision,
        })
    }

    /// Resolve known equivalent subdivision codes to the receiver's spelling.
    fn normalized(self, entity: Option<DxccEntity>, japan: &'static japan::Prefectures) -> Self {
        Self {
            subdivision: qualified_subdivision(self.country, self.subdivision, entity, japan),
            ..self
        }
    }

    /// Compare the country and subdivision of two normalized locations.
    fn same_location(self, other: Self) -> bool {
        self.country == other.country && self.subdivision.eq_ignore_ascii_case(other.subdivision)
    }
}

/// A POTA subdivision suffix is one to three ASCII letters or digits.
fn subdivision_code(value: &str) -> bool {
    (1..=3).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// Combine the exact park qualifier and applicable recorded primary subdivision.
///
/// A qualifier supplies the full location. Without either source the POTA
/// uploader can prompt, so the record omits MY_STATE and stays in its own
/// unknown-location partition. A present inconsistent fact needs review.
fn activation<'a>(
    park: &'a str,
    qualifier: Option<Qualification<'a>>,
    primary: Option<&'a str>,
    entity: Option<DxccEntity>,
    japan: &'static japan::Prefectures,
) -> Option<Activation<'a>> {
    let (program, _) = park.rsplit_once('-')?;
    let recorded_entity_country = entity.and_then(entity_country);
    let park_country = if entity.is_some() {
        recorded_entity_country
    } else {
        program_country(program)
    };
    let recorded = primary.and_then(|value| match Qualification::parse(value) {
        Some(Qualification {
            country,
            subdivision,
        }) => Some((Some(country), subdivision)),
        None if subdivision_code(value) => Some((None, value)),
        None => None,
    });
    let mut unverified_subdivision = primary.is_some() && recorded.is_none();
    let (location, subdivision) = match (qualifier, recorded) {
        (
            Some(Qualification {
                country,
                subdivision,
            }),
            recorded,
        ) => {
            if let Some((recorded_country, recorded_subdivision)) = recorded {
                if let Some(recorded_country) = recorded_country {
                    if recorded_country != country
                        || !qualified_subdivision(country, recorded_subdivision, entity, japan)
                            .eq_ignore_ascii_case(subdivision)
                    {
                        return None;
                    }
                } else {
                    let applicable_country = recorded_entity_country.unwrap_or(country);
                    let known_subdivision = primary_subdivision(
                        applicable_country,
                        entity,
                        recorded_subdivision,
                        japan,
                    );
                    if let Some(code) = known_subdivision {
                        // A numeric Japanese qualifier may still refer to several distinct
                        // POTA locations when its DXCC entity is unknown.
                        if applicable_country != country {
                            return None;
                        }
                        if country == "JP" && subdivision.bytes().all(|byte| byte.is_ascii_digit())
                        {
                            unverified_subdivision = true;
                        } else if !code.eq_ignore_ascii_case(subdivision) {
                            return None;
                        }
                    } else if !recorded_subdivision.eq_ignore_ascii_case(subdivision) {
                        unverified_subdivision = true;
                    }
                }
            }
            (format!("{country}-{subdivision}"), Some(subdivision))
        }
        (None, Some((country, subdivision))) => {
            let (country, subdivision) = if let Some(country) = country {
                (
                    Some(country),
                    qualified_subdivision(country, subdivision, entity, japan),
                )
            } else {
                (
                    park_country,
                    park_country
                        .and_then(|country| {
                            primary_subdivision(country, entity, subdivision, japan)
                        })
                        .unwrap_or(subdivision),
                )
            };
            (
                country.map_or_else(
                    || subdivision.into(),
                    |country| format!("{country}-{subdivision}"),
                ),
                Some(subdivision),
            )
        }
        (None, None) => (String::new(), None),
    };
    Some(Activation {
        park,
        location: location.to_ascii_uppercase(),
        subdivision,
        unverified_subdivision,
    })
}

/// Interpret an explicitly qualified POTA or ISO location within the recorded entity.
fn qualified_subdivision<'a>(
    country: &str,
    subdivision: &'a str,
    entity: Option<DxccEntity>,
    japan: &'static japan::Prefectures,
) -> &'a str {
    match (country, entity) {
        ("JP", Some(DxccEntity::Japan)) => japan
            .qualified_subdivision(subdivision)
            .unwrap_or(subdivision),
        ("MX", Some(DxccEntity::Mexico) | None) => {
            mexico_subdivision(subdivision).unwrap_or(subdivision)
        }
        _ => subdivision,
    }
}

/// Find the POTA equivalent of a bare code in the applicable subdivision system.
///
/// A country may use distinct ADIF and POTA identifiers. Lack of a known
/// correspondence leaves the qualified park as the usable location evidence.
fn primary_subdivision(
    country: &str,
    entity: Option<DxccEntity>,
    value: &str,
    japan: &'static japan::Prefectures,
) -> Option<&'static str> {
    match (country, entity) {
        ("JP", Some(DxccEntity::Japan) | None) => japan.primary_subdivision(value),
        ("MX", Some(DxccEntity::Mexico) | None) => mexico_subdivision(value),
        ("AU", Some(DxccEntity::Australia) | None) => value
            .parse::<australia::PrimarySubdivision>()
            .ok()
            .map(Into::into),
        ("US", Some(DxccEntity::UnitedStates | DxccEntity::Alaska | DxccEntity::Hawaii) | None) => {
            value
                .parse::<united_states::PrimarySubdivision>()
                .ok()
                .map(Into::into)
        }
        ("CA", Some(DxccEntity::Canada) | None) => value
            .parse::<canada::PrimarySubdivision>()
            .ok()
            .map(Into::into),
        ("DE", Some(DxccEntity::FederalRepublicOfGermany) | None) => value
            .parse::<germany::PrimarySubdivision>()
            .ok()
            .map(Into::into),
        _ => None,
    }
}

/// Resolve any current or import-only Mexican ADIF code to its current POTA spelling.
fn mexico_subdivision(value: &str) -> Option<&'static str> {
    use mexico::PrimarySubdivision::*;

    Some(
        match value.parse::<mexico::PrimarySubdivision>().ok()? {
            DistritoFederal => CiudadDeMexico,
            EstadoDeMexicoLegacy => Mexico,
            GuanajuatoLegacy => Guanajuato,
            HidalgoLegacy => Hidalgo,
            QueretaroDeArteaga => Queretaro,
            TlaxcalaLegacy => Tlaxcala,
            AguascalientesLegacy => Aguascalientes,
            BajaCaliforniaLegacy => BajaCalifornia,
            DurangoLegacy => Durango,
            NuevoLeonLegacy => NuevoLeon,
            TamaulipasLegacy => Tamaulipas,
            ChiapasLegacy => Chiapas,
            QuintanaRooLegacy => QuintanaRoo,
            current => current,
        }
        .into(),
    )
}

/// Collect distinct readiness problems and prepare the receiver's park facts.
///
/// Invalid own park references and conflicting own park locations require
/// review of the QSO. The optional contacted park may be omitted with a
/// warning while retaining the QSO.
pub(super) fn assess<'a>(
    contact: &'a ExportContact,
    group_id: GroupId,
    issues: &mut Vec<ReadinessField>,
    warnings: &mut Vec<ExportWarning>,
) -> anyhow::Result<ContactReadiness<'a>> {
    let japan = japan::Prefectures::bundled()?;
    let location = contact.qso.own_station().location.as_ref();
    let primary = location.and_then(|location| location.primary_subdivision().as_deref());
    let entity = location.and_then(|location| location.dxcc().as_deref()?.parse().ok());
    let mut parks = BTreeMap::new();
    let mut qualifiers = BTreeMap::new();
    let mut first_qualification = None;
    let mut invalid_park = false;
    let mut invalid_location = false;
    for value in contact
        .qso
        .own_station()
        .references
        .iter()
        .filter(|reference| reference.group_id == group_id)
        .filter_map(|reference| reference.identifier().as_deref())
    {
        let Some((park, qualifier)) = park_reference(value) else {
            invalid_park = true;
            continue;
        };
        if let Some(qualifier) = qualifier {
            let Some(qualification) = Qualification::parse(qualifier) else {
                invalid_location = true;
                continue;
            };
            let qualification = qualification.normalized(entity, japan);
            if !first_qualification
                .get_or_insert(qualification)
                .same_location(qualification)
            {
                invalid_location = true;
            }
            qualifiers.insert(park, qualification);
        }
        parks.insert(value, park);
    }
    let mut activations = BTreeMap::new();
    for (value, park) in parks {
        // An unqualified park can use this operating location when its program
        // identifies the same country.
        let qualifier = qualifiers.get(park).copied().or_else(|| {
            let (program, _) = park.rsplit_once('-')?;
            first_qualification
                .filter(|location| program_country(program) == Some(location.country))
        });
        if let Some(activation) = activation(park, qualifier, primary, entity, japan) {
            activations.insert(value, activation);
        } else {
            invalid_location = true;
        }
    }
    if invalid_park || (activations.is_empty() && !invalid_location) {
        issues.push(ReadinessField::OwnPark);
    }
    if invalid_location {
        issues.push(ReadinessField::ActivationLocation);
    }
    if activations
        .values()
        .any(|item| item.unverified_subdivision && item.subdivision.is_some())
    {
        warnings.push(ExportWarning::new(
            contact,
            "MY_STATE",
            "Used the park qualifier because the recorded subdivision could not be matched to a POTA location code.",
        ));
    }
    if activations
        .values()
        .any(|item| item.unverified_subdivision && item.subdivision.is_none())
    {
        warnings.push(ExportWarning::new(
            contact,
            "MY_STATE",
            "Omitted a recorded subdivision that could not be used as a POTA location code.",
        ));
    }

    let mut omitted = false;
    let mut contacted_parks = BTreeSet::new();
    let contacted_references = contact
        .contacted_references
        .iter()
        .filter(|reference| {
            if reference.group_id != group_id {
                return true;
            }
            if let Some(value) = reference.identifier().as_deref()
                && let Some((park, qualifier)) = park_reference(value)
                && qualifier.is_none_or(|location| Qualification::parse(location).is_some())
            {
                contacted_parks.insert(park);
                return true;
            }
            omitted = true;
            false
        })
        .cloned()
        .collect();
    if omitted {
        warnings.push(ExportWarning::new(
            contact,
            "SIG_INFO",
            "Omitted an unsupported contacted park identifier.",
        ));
    }

    Ok(ContactReadiness {
        activations,
        contacted_references,
        contacted_parks,
    })
}

/// Check the installed POTA rule's receiver fields against the chosen partition.
///
/// Apply the activation qualifier and exact applicable subdivision, when
/// available. A mismatch between the account plan and receiver's essential
/// output bindings is a configuration error.
pub(super) fn apply_partition(
    planned_fields: &mut AdifFields,
    activation: &Activation<'_>,
    planned_subdivision: &str,
) -> bool {
    if planned_fields.get("MY_SIG").map(String::as_str) != Some("POTA")
        || planned_fields.get("MY_SIG_INFO").map(String::as_str) != Some(activation.park)
        || planned_fields
            .get("MY_STATE")
            .map(String::as_str)
            .unwrap_or_default()
            != planned_subdivision
    {
        return false;
    }
    match activation.subdivision {
        Some(subdivision) => {
            planned_fields.insert("MY_STATE".into(), subdivision.to_ascii_uppercase());
        }
        None => {
            planned_fields.remove("MY_STATE");
        }
    }
    true
}

impl ContactReadiness<'_> {
    /// Validate the canonical row and identify the eligible park represented by it.
    pub(super) fn planned_contacted_park(
        &self,
        fields: &AdifFields,
    ) -> anyhow::Result<Option<String>> {
        match (
            fields.get("SIG").map(String::as_str),
            fields.get("SIG_INFO"),
        ) {
            (None, None) if self.contacted_parks.is_empty() => Ok(None),
            (Some("POTA"), Some(park)) if self.contacted_parks.contains(park.as_str()) => {
                Ok(Some(park.clone()))
            }
            _ => {
                anyhow::bail!("The installed POTA mapping supplies an inconsistent contacted park.")
            }
        }
    }
}

/// Supply the country for park programs whose reference prefix identifies it.
///
/// VE can mean either a legacy Canadian park or a current Venezuelan park.
fn program_country(program: &str) -> Option<&'static str> {
    match program {
        "US" | "K" => Some("US"),
        "CA" => Some("CA"),
        "MX" => Some("MX"),
        "JP" => Some("JP"),
        "AU" => Some("AU"),
        "DE" => Some("DE"),
        "UM" => Some("UM"),
        _ => None,
    }
}

/// The country part of POTA's subdivision codes for these DXCC entities.
fn entity_country(entity: DxccEntity) -> Option<&'static str> {
    match entity {
        DxccEntity::UnitedStates
        | DxccEntity::Alaska
        | DxccEntity::Hawaii
        | DxccEntity::KureIsland => Some("US"),
        DxccEntity::Canada => Some("CA"),
        DxccEntity::Mexico => Some("MX"),
        DxccEntity::Japan => Some("JP"),
        DxccEntity::Australia => Some("AU"),
        DxccEntity::FederalRepublicOfGermany => Some("DE"),
        DxccEntity::PalmyraAndJarvisIslands => Some("UM"),
        DxccEntity::Venezuela => Some("VE"),
        _ => None,
    }
}
