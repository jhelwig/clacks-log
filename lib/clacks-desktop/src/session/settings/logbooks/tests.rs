use crate::session::settings::logbooks::*;
use crate::session::{
    self, SessionCommand, SessionStatus,
    settings::{SettingsAction, SettingsRequest, SettingsResponse},
    tests::{Fixture, next_matching},
};
use clacks_client_store::{
    logbooks::{deletion::TagDeleteOutcome, selection::SelectedView},
    save::SaveOutcome,
};
use clacks_model::{
    ids::TagId,
    tag::{LogbookTag, TagDeletionChoice},
};
use std::time::Duration;
use tokio::sync::oneshot;

async fn request(
    sender: &tokio::sync::mpsc::Sender<SettingsRequest>,
    action: LogbookAction,
) -> anyhow::Result<SettingsResponse> {
    let (reply, response) = oneshot::channel();
    sender
        .send(SettingsRequest {
            action: SettingsAction::Logbooks(action),
            reply,
        })
        .await?;
    Ok(tokio::time::timeout(Duration::from_secs(15), response).await??)
}

#[tokio::test(flavor = "multi_thread")]
async fn session_logbook_operations_publish_committed_catalogs() -> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    let paths = fixture.resources.paths.clone();
    let sender = fixture.settings.clone();
    let view = fixture.settings_view.clone();
    let task = tokio::spawn(session::run(
        fixture.resources,
        fixture.input,
        fixture.updates,
    ));
    next_matching(&mut fixture.status, |state| {
        matches!(state, SessionStatus::Ready { .. })
    })
    .await?;
    let tag = LogbookTag::new(TagId::new(), "Main".into());
    assert!(matches!(
        request(
            &sender,
            LogbookAction::SaveTag {
                expected: None,
                next: tag.clone()
            }
        )
        .await?,
        SettingsResponse::Logbooks(LogbookResponse::Saved(outcome)) if matches!(*outcome, SaveOutcome::Saved(_))
    ));
    assert_eq!(
        view.borrow().as_ref().unwrap().logbooks.selected_view,
        SelectedView::AllContacts
    );
    assert!(
        matches!(request(&sender, LogbookAction::SelectView { next:SelectedView::Logbook(tag.id) }).await?, SettingsResponse::Logbooks(LogbookResponse::Selected(SelectedView::Logbook(id))) if id==tag.id)
    );
    let SettingsResponse::Logbooks(LogbookResponse::DeletionPreview(preview)) =
        request(&sender, LogbookAction::PreviewDeletion { id: tag.id }).await?
    else {
        panic!("preview");
    };
    assert_eq!(preview.counts.affected, 0);
    assert!(
        matches!(request(&sender, LogbookAction::DeleteTag { expected:preview, choice:TagDeletionChoice::LeaveAll }).await?, SettingsResponse::Logbooks(LogbookResponse::Deleted(TagDeleteOutcome::Applied { selected_view:SelectedView::AllContacts, fallback_from:Some(id), .. })) if id==tag.id)
    );
    assert_eq!(
        view.borrow().as_ref().unwrap().logbooks.selected_view,
        SelectedView::AllContacts
    );
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    task.await??;
    let reopened = crate::workspace::open(&paths).await?;
    assert_eq!(
        reopened.settings.logbooks.selected_view,
        SelectedView::AllContacts
    );
    assert!(matches!(
        reopened
            .settings
            .logbooks
            .tags
            .first()
            .unwrap()
            .working_state(),
        Some(clacks_sync::record::RecordState::Deleted)
    ));
    reopened.store.close().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn committed_logbook_outcome_survives_failed_followup_read() -> anyhow::Result<()> {
    use sqlx::{Connection, Executor};
    let mut fixture = Fixture::new()?;
    let paths = fixture.resources.paths.clone();
    let sender = fixture.settings.clone();
    let task = tokio::spawn(session::run(
        fixture.resources,
        fixture.input,
        fixture.updates,
    ));
    next_matching(&mut fixture.status, |state| {
        matches!(state, SessionStatus::Ready { .. })
    })
    .await?;
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&paths.database),
    )
    .await?;
    connection
        .execute(include_str!("../fail_refresh.sql"))
        .await?;
    connection.close().await?;
    let tag = LogbookTag::new(TagId::new(), "Saved despite refresh failure".into());
    let SettingsResponse::RefreshFailed { outcome, .. } = request(
        &sender,
        LogbookAction::SaveTag {
            expected: None,
            next: tag.clone(),
        },
    )
    .await?
    else {
        panic!("retain success");
    };
    let SettingsResponse::Logbooks(LogbookResponse::Saved(outcome)) = *outcome else {
        panic!("exact save result");
    };
    let SaveOutcome::Saved(row) = *outcome else {
        panic!("exact save result");
    };
    let store = clacks_client_store::store::ClientStore::open_discovered(
        &paths.database,
        Default::default(),
    )
    .await?;
    assert_eq!(
        clacks_client_store::save::load(&store, row.key).await?,
        Some(row)
    );
    store.close().await?;
    let SettingsResponse::RefreshFailed { outcome, .. } = request(
        &sender,
        LogbookAction::SelectView {
            next: SelectedView::Logbook(tag.id),
        },
    )
    .await?
    else {
        panic!("retain selection");
    };
    assert!(
        matches!(*outcome, SettingsResponse::Logbooks(LogbookResponse::Selected(SelectedView::Logbook(id))) if id==tag.id)
    );
    let SettingsResponse::RefreshFailed { outcome, .. } =
        request(&sender, LogbookAction::PreviewDeletion { id: tag.id }).await?
    else {
        panic!("retain preview");
    };
    let SettingsResponse::Logbooks(LogbookResponse::DeletionPreview(preview)) = *outcome else {
        panic!("preview");
    };
    let SettingsResponse::RefreshFailed { outcome, .. } = request(
        &sender,
        LogbookAction::DeleteTag {
            expected: preview,
            choice: TagDeletionChoice::LeaveAll,
        },
    )
    .await?
    else {
        panic!("retain deletion");
    };
    assert!(
        matches!(*outcome, SettingsResponse::Logbooks(LogbookResponse::Deleted(TagDeleteOutcome::Applied { selected_view:SelectedView::AllContacts, fallback_from:Some(id), .. })) if id==tag.id)
    );
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    task.await??;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn dismissed_logbook_requests_drain_during_shutdown() -> anyhow::Result<()> {
    use sqlx::Connection;
    let mut fixture = Fixture::new()?;
    let paths = fixture.resources.paths.clone();
    let sender = fixture.settings.clone();
    let task = tokio::spawn(session::run(
        fixture.resources,
        fixture.input,
        fixture.updates,
    ));
    next_matching(&mut fixture.status, |state| {
        matches!(state, SessionStatus::Ready { .. })
    })
    .await?;
    let mut blocker = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&paths.database),
    )
    .await?;
    let held = blocker.begin_with("BEGIN IMMEDIATE").await?;
    let tag = LogbookTag::new(TagId::new(), "Admitted".into());
    let (reply, response) = oneshot::channel();
    sender
        .send(SettingsRequest {
            action: SettingsAction::Logbooks(LogbookAction::SaveTag {
                expected: None,
                next: tag.clone(),
            }),
            reply,
        })
        .await?;
    drop(response);
    let permits = tokio::time::timeout(Duration::from_secs(15), sender.reserve_many(8)).await??;
    drop(permits);
    let (reply, queued) = oneshot::channel();
    sender
        .send(SettingsRequest {
            action: SettingsAction::Logbooks(LogbookAction::SelectView {
                next: SelectedView::Untagged,
            }),
            reply,
        })
        .await?;
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    next_matching(&mut fixture.status, |state| {
        matches!(state, SessionStatus::Closing { .. })
    })
    .await?;
    assert!(matches!(queued.await?, SettingsResponse::Closing));
    held.rollback().await?;
    blocker.close().await?;
    task.await??;
    let reopened = crate::workspace::open(&paths).await?;
    assert!(
        reopened
            .settings
            .logbooks
            .tags
            .iter()
            .any(|row| row.key.id == clacks_sync::record::RecordId::Tag(tag.id))
    );
    assert_eq!(
        reopened.settings.logbooks.selected_view,
        SelectedView::AllContacts
    );
    reopened.store.close().await?;
    Ok(())
}
