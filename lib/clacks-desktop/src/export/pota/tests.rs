use crate::export::{
    ExportFormat, ReadinessField,
    prepare::prepare,
    tests::{contact, input},
};
use clacks_client_store::read::export::ExportContact;
use clacks_model::location::RecordedLocationFields;
use std::collections::BTreeMap;

#[test]
fn own_parks_use_the_same_activation_location() -> anyhow::Result<()> {
    let conflicting = contact(&["US-0011@US-WA", "US-0012@US-OR"], &[])?;
    let conflicting_id = conflicting.qso.id();

    let mut equivalent = contact(&["JP-1252@JP-13", "JP-1255@JP-TK", "JP-1256"], &[])?;
    let equivalent_id = equivalent.qso.id();
    let mut fields = equivalent.qso.into_fields();
    fields.own_station.location = Some(
        RecordedLocationFields {
            dxcc: Some("339".into()),
            ..Default::default()
        }
        .try_into()?,
    );
    equivalent.qso = fields.try_into()?;
    let snapshot = input(vec![conflicting, equivalent])?;

    let plan = prepare(&snapshot, ExportFormat::Pota, &BTreeMap::new())?;

    let [conflict] = plan.skipped.as_slice() else {
        anyhow::bail!("the contact with contradictory own park locations requires review");
    };
    assert_eq!(conflict.qso_id, conflicting_id);
    assert_eq!(conflict.missing, [ReadinessField::ActivationLocation]);
    assert_eq!(plan.files.len(), 3);
    for park in ["JP-1252", "JP-1255", "JP-1256"] {
        let file = plan
            .files
            .iter()
            .find(|file| file.partition == [park, "JP-TK"])
            .ok_or_else(|| anyhow::anyhow!("missing output for {park} in Tokyo"))?;
        let [record] = file.records.as_slice() else {
            anyhow::bail!("each park file contains the contact made in Tokyo");
        };
        assert_eq!(record.qso_id, equivalent_id);
    }
    Ok(())
}

fn activation_contact(park: &str, location: Option<(&str, &str)>) -> anyhow::Result<ExportContact> {
    let mut contact = contact(&[park], &[])?;
    if let Some((entity, subdivision)) = location {
        let mut fields = contact.qso.into_fields();
        fields.own_station.location = Some(
            RecordedLocationFields {
                dxcc: Some(entity.into()),
                primary_subdivision: Some(subdivision.into()),
                ..Default::default()
            }
            .try_into()?,
        );
        contact.qso = fields.try_into()?;
    }
    Ok(contact)
}

#[test]
fn mexico_city_contacts_share_the_park_partition_across_recorded_spellings() -> anyhow::Result<()> {
    let snapshot = input(vec![
        activation_contact("MX-0125@MX-CMX", None)?,
        activation_contact("MX-0125", Some(("50", "CMX")))?,
        activation_contact("MX-0125", Some(("50", "DF")))?,
        activation_contact("MX-0125@MX-MOR", Some(("50", "MOR")))?,
    ])?;

    let plan = prepare(&snapshot, ExportFormat::Pota, &BTreeMap::new())?;

    assert!(plan.skipped.is_empty());
    assert_eq!(plan.files.len(), 2);
    for (location, subdivision, count) in [("MX-CMX", "CMX", 3), ("MX-MOR", "MOR", 1)] {
        let file = plan
            .files
            .iter()
            .find(|file| file.partition == ["MX-0125", location])
            .ok_or_else(|| anyhow::anyhow!("missing Mexican park output for {location}"))?;
        assert_eq!(file.records.len(), count);
        for record in &file.records {
            assert_eq!(
                record.fields.get("MY_STATE").map(String::as_str),
                Some(subdivision)
            );
        }
    }
    Ok(())
}

#[test]
fn tokyo_contact_uses_pota_location_with_the_equivalent_adif_prefecture() -> anyhow::Result<()> {
    let other_prefecture = activation_contact("JP-1255@JP-TK", Some(("339", "11")))?;
    let other_prefecture_id = other_prefecture.qso.id();
    let snapshot = input(vec![
        activation_contact("JP-1255@JP-TK", Some(("339", "10")))?,
        activation_contact("JP-1255", Some(("339", "10")))?,
        activation_contact("JP-1255@JP-13", Some(("339", "10")))?,
        other_prefecture,
    ])?;

    let plan = prepare(&snapshot, ExportFormat::Pota, &BTreeMap::new())?;

    let [conflict] = plan.skipped.as_slice() else {
        anyhow::bail!("only the contact whose recorded prefecture conflicts with Tokyo is skipped");
    };
    assert_eq!(conflict.qso_id, other_prefecture_id);
    assert_eq!(conflict.missing, [ReadinessField::ActivationLocation]);
    let [tokyo] = plan.files.as_slice() else {
        anyhow::bail!("the Tokyo contacts share one park file");
    };
    assert_eq!(tokyo.partition, ["JP-1255", "JP-TK"]);
    assert_eq!(tokyo.records.len(), 3);
    for record in &tokyo.records {
        assert_eq!(
            record.fields.get("MY_STATE").map(String::as_str),
            Some("TK")
        );
    }
    Ok(())
}

#[test]
fn australia_state_contacts_group_by_recorded_or_qualified_location() -> anyhow::Result<()> {
    let snapshot = input(vec![
        activation_contact("AU-7636@AU-NSW", None)?,
        activation_contact("AU-7636", Some(("150", "NSW")))?,
        activation_contact("AU-7636", Some(("150", "VIC")))?,
    ])?;

    let plan = prepare(&snapshot, ExportFormat::Pota, &BTreeMap::new())?;

    assert!(plan.skipped.is_empty());
    assert_eq!(plan.files.len(), 2);
    for (location, count) in [("AU-NSW", 2), ("AU-VIC", 1)] {
        let file = plan
            .files
            .iter()
            .find(|file| file.partition == ["AU-7636", location])
            .ok_or_else(|| anyhow::anyhow!("missing Australian park output for {location}"))?;
        assert_eq!(file.records.len(), count);
    }
    Ok(())
}

#[test]
fn germany_groups_equivalent_subdivision_codes_and_rejects_a_conflict() -> anyhow::Result<()> {
    let conflicting = activation_contact("DE-0001@DE-BY", Some(("230", "BW")))?;
    let conflicting_id = conflicting.qso.id();
    let snapshot = input(vec![
        activation_contact("DE-0001@DE-BY", None)?,
        activation_contact("DE-0001", Some(("230", "BY")))?,
        conflicting,
    ])?;

    let plan = prepare(&snapshot, ExportFormat::Pota, &BTreeMap::new())?;

    let [conflict] = plan.skipped.as_slice() else {
        anyhow::bail!("Bayern and Baden-Württemberg are distinct activation locations");
    };
    assert_eq!(conflict.qso_id, conflicting_id);
    assert_eq!(conflict.missing, [ReadinessField::ActivationLocation]);
    let [file] = plan.files.as_slice() else {
        anyhow::bail!("both contacts made in Bayern share the same park file");
    };
    assert_eq!(file.partition, ["DE-0001", "DE-BY"]);
    assert_eq!(file.records.len(), 2);
    for record in &file.records {
        assert_eq!(
            record.fields.get("MY_STATE").map(String::as_str),
            Some("BY")
        );
    }
    Ok(())
}

#[test]
fn opaque_optional_subdivision_uses_the_qualifier_or_leaves_location_unknown() -> anyhow::Result<()>
{
    let qualified = activation_contact("US-0011@US-WA", Some(("291", "Washington")))?;
    let qualified_id = qualified.qso.id();
    let bare = activation_contact("US-0011", Some(("291", "Washington")))?;
    let bare_id = bare.qso.id();
    let snapshot = input(vec![qualified, bare])?;

    let plan = prepare(&snapshot, ExportFormat::Pota, &BTreeMap::new())?;

    assert!(plan.skipped.is_empty());
    for id in [qualified_id, bare_id] {
        assert!(
            plan.warnings
                .iter()
                .any(|warning| warning.qso_id == id && warning.field == "MY_STATE")
        );
    }
    assert_eq!(plan.files.len(), 2);
    let unknown = plan
        .files
        .iter()
        .find(|file| file.partition == ["US-0011", ""])
        .ok_or_else(|| anyhow::anyhow!("missing output for the unknown location"))?;
    let washington = plan
        .files
        .iter()
        .find(|file| file.partition == ["US-0011", "US-WA"])
        .ok_or_else(|| anyhow::anyhow!("missing output for Washington"))?;
    let ([unknown], [washington]) = (unknown.records.as_slice(), washington.records.as_slice())
    else {
        anyhow::bail!("each location file contains its own contact");
    };
    assert_eq!(unknown.qso_id, bare_id);
    assert!(!unknown.fields.contains_key("MY_STATE"));
    assert_eq!(washington.qso_id, qualified_id);
    assert_eq!(
        washington.fields.get("MY_STATE").map(String::as_str),
        Some("WA")
    );
    Ok(())
}

#[test]
fn a_qualified_park_remains_usable_when_a_bare_code_cannot_be_compared() -> anyhow::Result<()> {
    let contact = activation_contact("VE-0044@VE-Z", Some(("148", "AM")))?;
    let id = contact.qso.id();
    let snapshot = input(vec![contact])?;

    let plan = prepare(&snapshot, ExportFormat::Pota, &BTreeMap::new())?;

    assert!(plan.skipped.is_empty());
    assert!(
        plan.warnings
            .iter()
            .any(|warning| warning.qso_id == id && warning.field == "MY_STATE")
    );
    let [file] = plan.files.as_slice() else {
        anyhow::bail!("the explicit qualifier identifies the Amazonas portion of the park");
    };
    assert_eq!(file.partition, ["VE-0044", "VE-Z"]);
    let [record] = file.records.as_slice() else {
        anyhow::bail!("the contact with the explicit qualifier is written");
    };
    assert_eq!(record.fields.get("MY_STATE").map(String::as_str), Some("Z"));
    Ok(())
}

#[test]
fn pota_historical_program_prefix_uses_recorded_region_to_group_activation() -> anyhow::Result<()> {
    let bare = activation_contact("VE-5082", Some(("1", "AB")))?;
    let validated_qualification = contact(&["VE-5082", "VE-5082@CA-AB"], &[])?;
    let snapshot = input(vec![bare, validated_qualification])?;

    let plan = prepare(&snapshot, ExportFormat::Pota, &BTreeMap::new())?;
    let [file] = plan.files.as_slice() else {
        anyhow::bail!("the two contacts specify the same park and Canadian location");
    };
    assert_eq!(file.partition, ["VE-5082", "CA-AB"]);
    assert_eq!(file.records.len(), 2);
    for record in &file.records {
        assert_eq!(
            record.fields.get("MY_STATE").map(String::as_str),
            Some("AB")
        );
    }

    Ok(())
}
