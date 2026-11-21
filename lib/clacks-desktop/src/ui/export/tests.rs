use super::*;
use crate::export::{
    PreparedFile, PreparedRecord, ReadinessField, SkippedContact,
    publish::{ExportReport, publish},
};
use clacks_client_store::read::export::{ExportContact, ExportCounts, ExportInput};
use clacks_model::query::Query;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tokio::sync::mpsc::error::TryRecvError;

#[test]
fn pota_export_reviews_skips_and_keeps_the_result_after_workspace_retirement() -> anyhow::Result<()>
{
    let (commands, mut requests) = mpsc::channel(4);
    let account = AccountId::new();
    let eligible = QsoId::new();
    let skipped = QsoId::new();
    let also_skipped = QsoId::new();
    let mut export = ExportUi::default();
    export.open(
        account,
        History {
            view: Default::default(),
            filter: Query::All,
            selected: [skipped, also_skipped].into(),
        },
        String::new(),
        "All contacts".into(),
        None,
        &commands,
    );
    let SessionCommand::Export(ordinary) = requests.try_recv()? else {
        anyhow::bail!("setup must submit preparation");
    };
    let frame = eframe::Frame::_new_kittest();
    let mut ui = Harness::builder().build_ui_state(
        |ui, export| {
            export.poll(ui.ctx(), &commands, Some(account));
            export.show(ui.ctx(), &commands, &frame);
        },
        export,
    );
    ui.get_by_label("POTA-compatible").click();
    ui.step();
    let SessionCommand::Export(pota) = requests.try_recv()? else {
        anyhow::bail!("changing format must prepare the new format");
    };
    assert!(ordinary.stop.is_cancelled());
    let capture = Arc::new(Capture {
        input: ExportInput {
            contacts: vec![],
            definitions: Default::default(),
            profiles: BTreeMap::new(),
        },
        workspace_stop: CancellationToken::new(),
        counts: ExportCounts {
            selected: 2,
            hidden: 1,
            matching: 2,
            all: 3,
        },
    });
    let lacking_park = vec![
        SkippedContact {
            qso_id: skipped,
            callsign: "K1SKIP".into(),
            missing: vec![ReadinessField::OwnPark],
        },
        SkippedContact {
            qso_id: also_skipped,
            callsign: "K2SKIP".into(),
            missing: vec![ReadinessField::OwnPark],
        },
    ];
    assert!(
        pota.reply
            .send(Ok(Reply::Prepared {
                capture: capture.clone(),
                plan: Arc::new(PreparedExport {
                    format: ExportFormat::Pota,
                    skipped: lacking_park.clone(),
                    requested_count: 2,
                    ..Default::default()
                }),
            }))
            .is_ok()
    );
    ui.run();
    ui.get_by_label("All 2 QSOs are missing required POTA information.");
    assert!(
        ui.get_by_role_and_label(egui::Role::Button, "Continue…")
            .accesskit_node()
            .is_disabled()
    );

    ui.get_by_label("All contacts in this workspace").click();
    ui.step();
    let SessionCommand::Export(all) = requests.try_recv()? else {
        anyhow::bail!("Changing the scope must request new POTA preparation");
    };
    assert!(matches!(
        all.action,
        Action::Prepare {
            scope: Scope::All,
            format: ExportFormat::Pota,
            ..
        }
    ));
    assert!(
        all.reply
            .send(Ok(Reply::Prepared {
                capture,
                plan: Arc::new(PreparedExport {
                    format: ExportFormat::Pota,
                    files: vec![PreparedFile {
                        partition: vec!["US-0011".into(), "US-WA".into()],
                        records: vec![PreparedRecord {
                            qso_id: eligible,
                            fields: [("CALL".into(), "K1KEEP".into())].into(),
                        }],
                    }],
                    skipped: lacking_park,
                    requested_count: 3,
                    ..Default::default()
                }),
            }))
            .is_ok()
    );
    ui.run();
    ui.get_by_label(
        "2 of 3 QSOs are missing required POTA information. Continuing will skip those 2.",
    );
    ui.get_by_label("Show affected QSOs").click();
    ui.run();
    ui.get_by_label("K1SKIP · Valid activation park (MY_SIG_INFO)");
    ui.get_by_role_and_label(egui::Role::Button, "Continue…")
        .click();
    ui.run();
    ui.get_by_label("Review POTA files");
    assert!(matches!(requests.try_recv(), Err(TryRecvError::Empty)));
    let reviewed_file = ui
        .get_by_label_contains("US-0011-US-WA_")
        .accesskit_node()
        .value()
        .ok_or_else(|| anyhow::anyhow!("the reviewed POTA file needs a visible name"))?;

    // Supply native picker completions at the UI boundary.
    ui.state_mut().picker = Some(Box::pin(std::future::ready(None)));
    ui.run();
    ui.get_by_label(&reviewed_file);
    assert!(matches!(requests.try_recv(), Err(TryRecvError::Empty)));

    let occupied = tempfile::tempdir()?;
    ui.state_mut().picker = Some(Box::pin(std::future::ready(Some(
        occupied.path().to_owned(),
    ))));
    ui.step();
    let SessionCommand::Export(write) = requests.try_recv()? else {
        anyhow::bail!("choosing a POTA folder must request publication");
    };
    let Action::Publish {
        destination, plan, ..
    } = write.action
    else {
        anyhow::bail!("publication must use the reviewed POTA plan");
    };
    let ExportDestination::Pota { directory, names } = &destination else {
        anyhow::bail!("POTA publication must use a folder");
    };
    assert_eq!(directory, occupied.path());
    let [file] = names.as_slice() else {
        anyhow::bail!("only the one eligible activation belongs in the export");
    };
    let name = file.name().to_owned();
    assert_eq!(reviewed_file, format!("{name} · 1 entry"));
    std::fs::write(occupied.path().join(&name), b"existing log")?;
    let outcome = publish(&plan, &destination, &write.stop);
    write
        .reply
        .send(Ok(Reply::Published(outcome)))
        .map_err(|_| anyhow::anyhow!("the export window must receive the collision result"))?;
    ui.run();
    ui.get_by_label("Export failed");
    ui.get_by_role_and_label(egui::Role::Button, "Choose another folder…");

    // The successful retry reply can arrive as the workspace stops accepting work.
    let folder = tempfile::tempdir()?;
    ui.state_mut().picker = Some(Box::pin(std::future::ready(Some(folder.path().to_owned()))));
    ui.step();
    let SessionCommand::Export(retry) = requests.try_recv()? else {
        anyhow::bail!("choosing another folder must request publication again");
    };
    let Action::Publish {
        destination: ExportDestination::Pota { directory, names },
        ..
    } = retry.action
    else {
        anyhow::bail!("the retry must publish the reviewed POTA files");
    };
    assert_eq!(directory, folder.path());
    let [file] = names.as_slice() else {
        anyhow::bail!("the retry must keep the eligible activation in the review");
    };
    assert_eq!(file.name(), name);
    retry
        .reply
        .send(Ok(Reply::Published(Ok(ExportReport {
            paths: vec![folder.path().join(name)],
            entries_written: 1,
            qsos_exported: 1,
        }))))
        .map_err(|_| anyhow::anyhow!("the export window must receive the completed result"))?;
    let context = ui.ctx.clone();
    ui.state_mut().poll(&context, &commands, None);
    ui.run();
    ui.get_by_label("Export complete");
    ui.get_by_label("1 QSO exported · 2 skipped · 1 entry · 1 file");
    ui.get_by_role_and_label(egui::Role::Button, "Done").click();
    ui.run();
    assert!(ui.query_by_label("Export complete").is_none());
    Ok(())
}

#[test]
fn destination_return_after_workspace_retirement_submits_no_write() -> anyhow::Result<()> {
    let (commands, mut requests) = mpsc::channel(2);
    let account = AccountId::new();
    let context = egui::Context::default();
    let mut export = ExportUi::default();
    export.open(
        account,
        History {
            view: Default::default(),
            filter: Query::All,
            selected: Default::default(),
        },
        String::new(),
        "All contacts".into(),
        None,
        &commands,
    );
    let SessionCommand::Export(preparation) = requests.try_recv()? else {
        anyhow::bail!("Export must prepare the working snapshot");
    };
    let workspace_stop = CancellationToken::new();
    assert!(
        preparation
            .reply
            .send(Ok(Reply::Prepared {
                capture: Arc::new(Capture {
                    input: ExportInput {
                        contacts: vec![],
                        definitions: Default::default(),
                        profiles: BTreeMap::new()
                    },
                    workspace_stop: workspace_stop.clone(),
                    counts: ExportCounts {
                        selected: 0,
                        hidden: 0,
                        matching: 1,
                        all: 1
                    },
                }),
                plan: Arc::new(PreparedExport {
                    requested_count: 1,
                    files: vec![PreparedFile {
                        partition: vec![],
                        records: vec![PreparedRecord {
                            qso_id: QsoId::new(),
                            fields: [("CALL".into(), "K1KEEP".into())].into(),
                        }],
                    }],
                    ..Default::default()
                }),
            }))
            .is_ok()
    );
    export.poll(&context, &commands, Some(account));
    export.picker = Some(Box::pin(std::future::ready(Some("old-account.adi".into()))));
    // The status receiver can miss Opening when Ready resumes for the same account.
    workspace_stop.cancel();
    export.poll(&context, &commands, Some(account));
    assert!(!export.is_open());
    assert!(matches!(requests.try_recv(), Err(TryRecvError::Empty)));
    Ok(())
}

#[test]
fn ordinary_export_uses_the_reviewed_contest_choice_and_requires_replacement_consent()
-> anyhow::Result<()> {
    use clacks_model::{export::select::ExportDecision, ids::GroupId};

    let (commands, mut requests) = mpsc::channel(4);
    let account = AccountId::new();
    let qso = crate::ui::tests::fixtures::contact();
    let contact = qso.id();
    let mut export = ExportUi::default();
    export.open(
        account,
        History {
            view: Default::default(),
            filter: Query::All,
            selected: [contact].into(),
        },
        String::new(),
        "All contacts".into(),
        None,
        &commands,
    );
    let SessionCommand::Export(initial) = requests.try_recv()? else {
        anyhow::bail!("The dialog must request a plan before showing an ordinary export");
    };
    let capture = Arc::new(Capture {
        input: ExportInput {
            contacts: vec![ExportContact {
                qso: qso.clone(),
                facts: Default::default(),
                contacted_references: Default::default(),
                confirmation_dates: vec![],
            }],
            definitions: Default::default(),
            profiles: BTreeMap::new(),
        },
        workspace_stop: CancellationToken::new(),
        counts: ExportCounts {
            selected: 1,
            hidden: 0,
            matching: 1,
            all: 1,
        },
    });
    assert!(
        initial
            .reply
            .send(Ok(Reply::Prepared {
                capture: capture.clone(),
                plan: Arc::new(PreparedExport {
                    requested_count: 1,
                    decisions: vec![ExportDecision::AmbiguousContextField {
                        qso_id: contact,
                        field: "CONTEST_ID".into(),
                        candidates: [
                            (GroupId::new(), "ARRL-DX-CW".into()),
                            (GroupId::new(), "CQ-WW-CW".into())
                        ]
                        .into(),
                    }],
                    ..Default::default()
                }),
            }))
            .is_ok()
    );
    let frame = eframe::Frame::_new_kittest();
    let mut ui = Harness::builder().build_ui_state(
        |ui, export| {
            export.poll(ui.ctx(), &commands, Some(account));
            export.show(ui.ctx(), &commands, &frame);
        },
        export,
    );
    ui.get_by_label_contains("· CONTEST_ID").click();
    ui.run();
    ui.get_by_label("Omit contest identifier").click();
    ui.step();
    let SessionCommand::Export(resolved) = requests.try_recv()? else {
        anyhow::bail!("Choosing omission must request a resolved plan");
    };
    let Action::Prepare {
        capture: Some(reviewed),
        resolutions,
        ..
    } = resolved.action
    else {
        anyhow::bail!("The contest choice must reuse the reviewed snapshot");
    };
    let [reviewed_contact] = reviewed.input.contacts.as_slice() else {
        anyhow::bail!("resolving the contest must carry the saved contact");
    };
    assert_eq!(reviewed_contact.qso, qso);
    assert_eq!(
        resolutions,
        [(contact, ContextResolution::OmitReported)].into()
    );
    assert!(
        resolved
            .reply
            .send(Ok(Reply::Prepared {
                capture,
                plan: Arc::new(PreparedExport {
                    requested_count: 1,
                    files: vec![PreparedFile {
                        partition: vec![],
                        records: vec![PreparedRecord {
                            qso_id: contact,
                            fields: [(
                                "CALL".into(),
                                qso.contacted_station().callsign.as_str().into()
                            )]
                            .into(),
                        }]
                    }],
                    ..Default::default()
                }),
            }))
            .is_ok()
    );
    ui.run();

    let folder = tempfile::tempdir()?;
    let path = folder.path().join("existing.adi");
    std::fs::write(&path, "prior export")?;
    ui.state_mut().picker = Some(Box::pin(std::future::ready(Some(path.clone()))));
    ui.step();
    let SessionCommand::Export(unconfirmed) = requests.try_recv()? else {
        anyhow::bail!("selecting an ordinary filename must request publication");
    };
    let Action::Publish {
        plan, destination, ..
    } = unconfirmed.action
    else {
        anyhow::bail!("the Save dialog path must accompany the reviewed plan");
    };
    assert!(matches!(&destination, ExportDestination::Ordinary {
        path: selected, replace: false,
    } if selected == &path));
    let outcome = publish(&plan, &destination, &unconfirmed.stop);
    unconfirmed
        .reply
        .send(Ok(Reply::Published(outcome)))
        .map_err(|_| anyhow::anyhow!("the export window must receive the collision result"))?;
    ui.run();

    ui.get_by_role_and_label(egui::Role::Button, "Replace file")
        .click();
    ui.step();
    let SessionCommand::Export(confirmed) = requests.try_recv()? else {
        anyhow::bail!("confirming replacement must submit another publication request");
    };
    assert!(matches!(confirmed.action, Action::Publish {
        destination: ExportDestination::Ordinary { path: selected, replace: true },
        ..
    } if selected == path));
    Ok(())
}
