//! Headless tests of the production session with real workspace resources.

mod logbooks;
mod settings;

use super::*;
use crate::{
    config::{self, Cli, StartupContext},
    instance::{self, Election},
    workspace::AppDirectories,
};
use clap::Parser;

pub(crate) struct Fixture {
    pub(crate) _directory: tempfile::TempDir,
    pub(crate) _owner: instance::InstanceOwner,
    pub(crate) resources: SessionResources,
    pub(crate) status: watch::Receiver<SessionStatus>,
    pub(crate) updates: watch::Sender<SessionStatus>,
    pub(crate) commands: mpsc::Sender<SessionCommand>,
    pub(crate) input: mpsc::Receiver<SessionCommand>,
    pub(crate) settings: mpsc::Sender<super::settings::SettingsRequest>,
    pub(crate) lookup: watch::Receiver<Option<crate::directory::lookup::LookupClient>>,
    pub(crate) settings_view:
        watch::Receiver<Option<Arc<clacks_client_store::settings::SettingsSnapshot>>>,
}

impl Fixture {
    pub(crate) fn new() -> anyhow::Result<Self> {
        let directory = tempfile::tempdir()?;
        Self::with_directory(directory)
    }

    /// Reopen the same real workspace after its prior session and owner have stopped.
    pub(crate) fn with_directory(directory: tempfile::TempDir) -> anyhow::Result<Self> {
        let config = config::resolve(
            Cli::try_parse_from(["clacks-log"])?,
            &StartupContext {
                cwd: directory.path().into(),
                environment: ::config::Environment::default().source(Some(Default::default())),
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
        let (settings, requests) = mpsc::channel(8);
        let (settings_updates, settings_view) = watch::channel(None);
        let (lookup_output, lookup) = watch::channel(None);
        let resources = SessionResources {
            geography: watch::channel(Default::default()).0,
            geography_bundle: None,
            ws_suite: crate::ws_suite::Outputs {
                calls: tokio::sync::mpsc::unbounded_channel().0,
                updates: watch::channel(Default::default()).0,
                context: Default::default(),
            },
            lookup: lookup_output,
            contact_changes: tokio::sync::watch::channel(()).0,
            directory: crate::directory::provisioning::Outputs {
                installed: tokio::sync::watch::channel(None).0,
                status: tokio::sync::watch::channel(Default::default()).0,
                retry: Default::default(),
            },
            enrollment_updates: watch::channel(Default::default()).0,
            settings_requests: requests,
            settings_updates,
            paths,
            listener,
            telemetry,
            activation,
            context: Arc::new(OnceLock::new()),
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
            settings,
            lookup,
            settings_view,
        })
    }
}

pub(crate) async fn next_matching(
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
async fn retry_does_not_leak_failed_resources() -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    fixture.resources.geography_bundle = Some(crate::geography::Selection {
        path: fixture._directory.path().join("missing-override"),
        manifest_sha256: "0".repeat(64),
    });
    let mut geography = fixture.resources.geography.subscribe();
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
    tokio::time::timeout(Duration::from_secs(5), async {
        while matches!(
            *geography.borrow_and_update(),
            crate::geography::GeographyStatus::Opening
        ) {
            geography.changed().await?;
        }
        anyhow::Ok(())
    })
    .await??;
    assert!(matches!(
        *geography.borrow(),
        crate::geography::GeographyStatus::Unavailable(_)
    ));
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

pub(crate) mod ws_suite;
