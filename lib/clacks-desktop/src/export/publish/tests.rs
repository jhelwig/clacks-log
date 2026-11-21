use super::*;
use crate::export::{PreparedFile, PreparedRecord};

use qso_io::adif::adi::parser::{
    options::ParseOpts,
    strict::{Section, StrictParser, record::Record as ParsedRecord},
};

fn ordinary_plan(fields: &[(&str, &str)]) -> PreparedExport {
    PreparedExport {
        format: ExportFormat::Ordinary,
        files: vec![PreparedFile {
            partition: Vec::new(),
            records: vec![PreparedRecord {
                qso_id: QsoId::new(),
                fields: fields
                    .iter()
                    .map(|&(name, value)| (name.into(), value.into()))
                    .collect(),
            }],
        }],
        requested_count: 1,
        ..Default::default()
    }
}

/// Give each park distinguishable output for the same activating QSO.
fn pota_plan(parks: &[(&str, &str)]) -> PreparedExport {
    let qso = QsoId::new();
    PreparedExport {
        format: ExportFormat::Pota,
        files: parks
            .iter()
            .map(|&(park, location)| PreparedFile {
                partition: vec![park.into(), location.into()],
                records: vec![PreparedRecord {
                    qso_id: qso,
                    fields: [
                        ("CALL".into(), "W7HEX/P".into()),
                        ("MY_SIG".into(), "POTA".into()),
                        ("MY_SIG_INFO".into(), park.into()),
                    ]
                    .into(),
                }],
            })
            .collect(),
        requested_count: 1,
        ..Default::default()
    }
}

/// Read exported entries without depending on the writer's line breaks or field order.
fn exported_records(adi: &[u8]) -> anyhow::Result<Vec<ParsedRecord<'_>>> {
    let mut parser = StrictParser::new(adi, ParseOpts::default());
    let mut records = Vec::new();
    for section in &mut parser {
        if let Section::Record(record) =
            section.map_err(|error| anyhow::anyhow!("Could not read the exported ADI: {error}"))?
        {
            records.push(record);
        }
    }
    Ok(records)
}

/// Publish all reviewed partitions and count their shared QSO once.
#[test]
fn pota_review_names_match_published_adi() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let parks = [("US-0011", "US-WA"), ("US-0012", "US-WA"), ("US-0013", "")];
    let plan = pota_plan(&parks);
    let names = file_names(&plan, UtcSecond::from_unix_seconds(1_799))?;
    assert_eq!(
        names.iter().map(PlannedFileName::name).collect::<Vec<_>>(),
        [
            "US-0011-US-WA_19700101T002959Z.adi",
            "US-0012-US-WA_19700101T002959Z.adi",
            "US-0013_19700101T002959Z.adi",
        ]
    );
    assert!(names.iter().all(|file| file.entry_count() == 1));
    let paths = names
        .iter()
        .map(|file| dir.path().join(file.name()))
        .collect::<Vec<_>>();

    let destination = ExportDestination::Pota {
        directory: dir.path().to_owned(),
        names,
    };
    let report = publish(&plan, &destination, &CancellationToken::new())?;

    assert_eq!(report.paths, paths);
    assert_eq!(report.entries_written, 3);
    assert_eq!(report.qsos_exported, 1);
    for (path, (park, _)) in paths.iter().zip(parks) {
        let adi = fs::read(path)?;
        let records = exported_records(&adi)?;
        let [record] = records.as_slice() else {
            anyhow::bail!("each reviewed park has one exported QSO");
        };
        assert_eq!(
            record.field("MY_SIG_INFO")?.map(|field| field.text()),
            Some(park)
        );
    }
    assert_eq!(fs::read_dir(dir.path())?.count(), 3);
    Ok(())
}

/// An invalid prepared field must fail before an approved replacement is applied.
#[test]
fn invalid_staged_adi_preserves_existing_ordinary_destination() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("contacts.adi");
    fs::write(&path, b"earlier export")?;
    let plan = ordinary_plan(&[("CALL", "W7HEX/P"), ("UNSUPPORTED_FIELD", "value")]);
    let destination = ExportDestination::Ordinary {
        path: path.clone(),
        replace: true,
    };

    let error = publish(&plan, &destination, &CancellationToken::new())
        .expect_err("Clacks cannot bind the unsupported prepared field to an ADI field");

    assert!(error.to_string().contains("UNSUPPORTED_FIELD"));
    assert!(error.report().paths.is_empty());
    assert_eq!(fs::read(&path)?, b"earlier export");
    assert_eq!(fs::read_dir(dir.path())?.count(), 1);
    Ok(())
}

/// Require replacement consent and serialize supported service application fields.
#[test]
fn confirmed_ordinary_replacement_writes_qrz_confirmation() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("contacts.adi");
    fs::write(&path, b"earlier export")?;
    let plan = ordinary_plan(&[
        ("CALL", "W7HEX/P"),
        ("APP_QRZLOG_STATUS", "C"),
        ("APP_QRZLOG_QSLDATE", "20261120"),
    ]);
    let unconfirmed = ExportDestination::Ordinary {
        path: path.clone(),
        replace: false,
    };
    let cancel = CancellationToken::new();
    let error = publish(&plan, &unconfirmed, &cancel)
        .expect_err("an existing ordinary ADI file needs replacement consent");
    assert!(error.is_collision());
    assert_eq!(fs::read(&path)?, b"earlier export");

    let confirmed = ExportDestination::Ordinary {
        path: path.clone(),
        replace: true,
    };
    let report = publish(&plan, &confirmed, &cancel)?;
    let adi = fs::read(&path)?;

    assert_eq!(report.paths, [path]);
    let records = exported_records(&adi)?;
    let [record] = records.as_slice() else {
        anyhow::bail!("the ordinary replacement contains one QSO");
    };
    for (name, data_type, value) in [
        ("APP_QRZLOG_QSLDATE", DataType::Date, "20261120"),
        ("APP_QRZLOG_STATUS", DataType::String, "C"),
    ] {
        assert_eq!(
            record
                .field(name)?
                .map(|field| (field.data_type(), field.text())),
            Some((Some(data_type), value))
        );
    }
    Ok(())
}

/// Cancellation after writing staging files must drop them without replacing an export.
#[test]
fn cancellation_at_publication_boundary_preserves_existing_ordinary_destination()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("contacts.adi");
    fs::write(&path, b"earlier export")?;
    let plan = ordinary_plan(&[("CALL", "W7HEX/P")]);
    let destination = ExportDestination::Ordinary {
        path: path.clone(),
        replace: true,
    };
    let cancel = CancellationToken::new();
    let staged = stage(&plan, &destination, &cancel)?;

    cancel.cancel();
    let error = staged
        .finish(&cancel)
        .expect_err("the export was cancelled before publication");

    assert!(error.is_cancelled());
    assert!(error.report().paths.is_empty());
    assert_eq!(fs::read(&path)?, b"earlier export");
    assert_eq!(fs::read_dir(dir.path())?.count(), 1);
    Ok(())
}

/// A collision after staging preserves the occupied destination and reports completed output.
#[test]
fn late_collision_reports_completed_output_and_preserves_external_file() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut plan = pota_plan(&[("US-0011", "US-WA"), ("US-0012", "US-WA")]);
    let [completed, blocked] = plan.files.as_mut_slice() else {
        anyhow::bail!("the two parks each have a prepared file");
    };
    let [contact] = completed.records.as_mut_slice() else {
        anyhow::bail!("the first park has a contact to expand");
    };
    contact.fields.extend([
        ("SIG".into(), "POTA".into()),
        ("SIG_INFO".into(), "US-1234".into()),
    ]);
    let mut expanded = contact.clone();
    expanded.fields.insert("SIG_INFO".into(), "US-1235".into());
    completed.records.push(expanded);
    let [contact] = blocked.records.as_mut_slice() else {
        anyhow::bail!("the second park has a contact of its own");
    };
    contact.qso_id = QsoId::new();
    contact.fields.insert("CALL".into(), "K1OTHER".into());
    plan.requested_count = 2;
    let names = file_names(&plan, UtcSecond::from_unix_seconds(0))?;
    let [first, second] = names.as_slice() else {
        anyhow::bail!("the two activation partitions need a destination each");
    };
    let first = dir.path().join(first.name());
    let second = dir.path().join(second.name());
    let destination = ExportDestination::Pota {
        directory: dir.path().to_owned(),
        names,
    };
    let cancel = CancellationToken::new();
    let staged = stage(&plan, &destination, &cancel)?;

    fs::write(&second, b"concurrent export")?;
    let error = staged
        .finish(&cancel)
        .expect_err("the second destination now exists");

    assert!(error.is_collision());
    assert_eq!(error.report().paths, std::slice::from_ref(&first));
    assert_eq!(error.report().entries_written, 2);
    assert_eq!(error.report().qsos_exported, 1);
    let adi = fs::read(&first)?;
    assert_eq!(exported_records(&adi)?.len(), 2);
    assert_eq!(fs::read(&second)?, b"concurrent export");
    assert_eq!(fs::read_dir(dir.path())?.count(), 2);
    Ok(())
}
