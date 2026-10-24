//! Headless tests of the production session with real workspace resources.

use super::*;
use crate::{
    config::{self, Cli, Environment},
    instance::{self, Election},
    workspace::AppDirectories,
};
use clap::Parser;

struct Fixture {
    _directory: tempfile::TempDir,
    _owner: instance::InstanceOwner,
    resources: SessionResources,
    status: watch::Receiver<SessionStatus>,
    updates: watch::Sender<SessionStatus>,
    commands: mpsc::Sender<SessionCommand>,
    input: mpsc::Receiver<SessionCommand>,
}

impl Fixture {
    fn new() -> anyhow::Result<Self> {
        let directory = tempfile::tempdir()?;
        let config = config::resolve(
            Cli::try_parse_from(["clacks-log"])?,
            &Environment {
                cwd: directory.path().into(),
                values: Default::default(),
                directories: AppDirectories {
                    config: directory.path().join("config"),
                    data: directory.path().join("data"),
                },
            },
        )?;
        let paths = WorkspacePaths::resolve(&config)?;
        let Election::Owner(owner) = instance::elect(&paths)? else {
            anyhow::bail!("owner expected")
        };
        let (notices, activation) = watch::channel(());
        let listener = owner.listen(&tokio::runtime::Handle::current(), notices)?;
        let telemetry = crate::telemetry::initialize(&config, &paths);
        let resources = SessionResources {
            paths,
            listener,
            telemetry,
            activation,
            context: Arc::new(OnceLock::new()),
            opening_gate: None,
        };
        let (updates, status) = watch::channel(SessionStatus::Opening);
        let (commands, input) = mpsc::channel(8);
        Ok(Self {
            _directory: directory,
            _owner: owner,
            resources,
            status,
            updates,
            commands,
            input,
        })
    }
}

async fn next_matching(
    status: &mut watch::Receiver<SessionStatus>,
    predicate: impl Fn(&SessionStatus) -> bool,
) -> anyhow::Result<SessionStatus> {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let current = status.borrow_and_update().clone();
            if predicate(&current) {
                return Ok(current);
            }
            status.changed().await?;
        }
    })
    .await?
}

#[tokio::test(flavor = "multi_thread")]
async fn close_during_open_does_not_publish_ready() -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    let paths = fixture.resources.paths.clone();
    let (release, gate) = tokio::sync::oneshot::channel();
    fixture.resources.opening_gate = Some(gate);
    let task = tokio::spawn(run(fixture.resources, fixture.input, fixture.updates));
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    let closing = next_matching(&mut fixture.status, |s| {
        matches!(s, SessionStatus::Closing { .. })
    })
    .await?;
    let SessionStatus::Closing { started, .. } = closing else {
        unreachable!()
    };
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    release.send(()).unwrap();
    loop {
        let current = fixture.status.borrow_and_update().clone();
        assert!(!matches!(current, SessionStatus::Ready { .. }));
        if let SessionStatus::Closing {
            started: current_start,
            ..
        } = current
        {
            assert_eq!(current_start, started);
        }
        if matches!(current, SessionStatus::Closed) {
            break;
        }
        fixture.status.changed().await?;
    }
    task.await??;
    // Account provisioning admitted before close must retain all native saves.
    let reopened = crate::workspace::open(&paths).await?;
    let definitions = clacks_model::defaults::account_definitions()?;
    for group in definitions.groups {
        let key = clacks_sync::record::RecordKey {
            account: reopened.identity.account,
            id: clacks_sync::record::RecordId::Group(group.id),
        };
        let saved = clacks_client_store::save::load(&reopened.store, key)
            .await?
            .unwrap();
        assert_eq!(
            saved.working_state(),
            Some(&clacks_sync::record::RecordState::Live(
                clacks_sync::record::RecordValue::Group(Box::new(group))
            ))
        );
    }
    reopened.store.close().await?;
    drop(fixture._owner);
    assert!(matches!(instance::elect(&paths)?, Election::Owner(_)));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn retry_does_not_leak_failed_resources() -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    std::fs::write(&fixture.resources.paths.database, b"invalid")?;
    let database = fixture.resources.paths.database.clone();
    let task = tokio::spawn(run(fixture.resources, fixture.input, fixture.updates));
    next_matching(&mut fixture.status, |s| {
        matches!(s, SessionStatus::Failed { .. })
    })
    .await?;
    assert_eq!(std::fs::read(&database)?, b"invalid");
    std::fs::remove_file(&database)?;
    fixture.commands.send(SessionCommand::RetryOpen).await?;
    next_matching(&mut fixture.status, |s| {
        matches!(s, SessionStatus::Ready { .. })
    })
    .await?;
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    next_matching(&mut fixture.status, |s| matches!(s, SessionStatus::Closed)).await?;
    task.await??;
    let store =
        clacks_client_store::store::ClientStore::open_discovered(&database, Default::default())
            .await?;
    store.close().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn disconnected_window_still_drains_open_and_closes() -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    let paths = fixture.resources.paths.clone();
    drop(fixture.commands);
    tokio::time::timeout(
        Duration::from_secs(15),
        run(fixture.resources, fixture.input, fixture.updates),
    )
    .await??;
    assert!(matches!(
        *fixture.status.borrow_and_update(),
        SessionStatus::Closed
    ));
    let opened = crate::workspace::open(&paths).await?;
    opened.store.close().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn close_while_failed_preserves_file_and_releases_resources() -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    let paths = fixture.resources.paths.clone();
    std::fs::write(&paths.database, b"incomplete")?;
    let task = tokio::spawn(run(fixture.resources, fixture.input, fixture.updates));
    next_matching(&mut fixture.status, |s| {
        matches!(s, SessionStatus::Failed { .. })
    })
    .await?;
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    next_matching(&mut fixture.status, |s| matches!(s, SessionStatus::Closed)).await?;
    task.await??;
    assert_eq!(std::fs::read(&paths.database)?, b"incomplete");
    drop(fixture._owner);
    assert!(matches!(instance::elect(&paths)?, Election::Owner(_)));
    Ok(())
}

/// Manual Windows acceptance uses the production UI/session with a private pause.
#[cfg(windows)]
#[test]
#[ignore = "interactive Windows acceptance; requires a visible native desktop"]
fn native_delayed_shutdown() -> anyhow::Result<()> {
    use winit::platform::windows::EventLoopBuilderExtWindows;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let mut fixture = {
        let _entered = runtime.enter();
        Fixture::new()?
    };
    let _retained_directory = fixture._directory.keep();
    let existing = runtime.block_on(crate::workspace::open(&fixture.resources.paths))?;
    runtime.block_on(existing.store.close())?;
    println!(
        "Native acceptance workspace: {}",
        fixture.resources.paths.directory.display()
    );
    println!(
        "Close the window while opening. After five seconds choose Continue Waiting (cleanup completes 20 seconds after close), or Force Quit (expected test-process exit 1). Reopen the printed workspace afterward."
    );
    let (release, gate) = tokio::sync::oneshot::channel();
    fixture.resources.opening_gate = Some(gate);
    let context = fixture.resources.context.clone();
    let paths = fixture.resources.paths.clone();
    let notices = fixture.resources.activation.clone();
    let diagnostics = fixture.resources.telemetry.diagnostics().to_vec();
    let loss = fixture.resources.telemetry.loss_counter();
    let file_failure = fixture.resources.telemetry.file_failure();
    let mut observing = fixture.status.clone();
    let release_task = runtime.spawn(async move {
        loop {
            if matches!(
                *observing.borrow_and_update(),
                SessionStatus::Closing { .. }
            ) {
                break;
            }
            observing.changed().await?;
        }
        tokio::time::sleep(Duration::from_secs(20)).await;
        let _ = release.send(());
        anyhow::Ok(())
    });
    let task = runtime.spawn(run(fixture.resources, fixture.input, fixture.updates));
    let commands = fixture.commands.clone();
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        event_loop_builder: Some(Box::new(|builder| {
            builder.with_any_thread(true);
        })),
        ..Default::default()
    };
    let window = eframe::run_native(
        "Clacks Log — delayed shutdown acceptance",
        options,
        Box::new(move |creation| {
            let _ = context.set(creation.egui_ctx.clone());
            Ok(Box::new(crate::ui::DesktopApp::new(
                fixture.status,
                notices,
                commands,
                paths,
                diagnostics,
                loss,
                file_failure,
            )))
        }),
    );
    runtime.block_on(async {
        let _ = fixture.commands.send(SessionCommand::BeginShutdown).await;
        task.await??;
        release_task.await??;
        anyhow::Ok(())
    })?;
    window.map_err(|error| anyhow::anyhow!("native acceptance window failed: {error}"))?;
    Ok(())
}
