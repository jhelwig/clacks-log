use crate::session::settings::apply::{AccountChanges, AccountOutcome, Edit};
use crate::session::settings::logbooks::*;
use crate::session::{
    self, SessionCommand, SessionStatus,
    settings::{SettingsAction, SettingsRequest, SettingsResponse},
    tests::{Fixture, next_matching},
};
use clacks_client_store::logbooks::selection::SelectedView;
use clacks_model::{
    ids::TagId,
    tag::{LogbookTag, TagDeletionChoice},
};
use clacks_sync::record::RecordId;
use std::time::Duration;
use tokio::sync::{oneshot, watch};

async fn request(
    sender: &tokio::sync::mpsc::Sender<SettingsRequest>,
    action: SettingsAction,
) -> anyhow::Result<SettingsResponse> {
    let (reply, response) = oneshot::channel();
    sender.send(SettingsRequest { action, reply }).await?;
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
    let mut changes = AccountChanges::default();
    changes.tags.insert(
        tag.id,
        Edit {
            expected: None,
            next: tag.clone(),
        },
    );
    assert!(matches!(
        request(
            &sender,
            SettingsAction::ApplyAccountChanges(changes.clone())
        )
        .await?,
        SettingsResponse::Failed(_)
    ));
    let opening = session::SettingsSessionId(1);
    fixture.settings_intent.send_replace(Some(opening));
    tokio::time::timeout(
        Duration::from_secs(5),
        fixture
            .settings_basis
            .wait_for(|basis| basis.as_ref().is_some_and(|basis| basis.id == opening)),
    )
    .await??;
    let SettingsResponse::ApplyAccountChanges(created) =
        request(&sender, SettingsAction::ApplyAccountChanges(changes)).await?
    else {
        anyhow::bail!("ready Settings should return the account operation results")
    };
    assert!(matches!(
        created.outcomes.get(&RecordId::Tag(tag.id)),
        Some(AccountOutcome::Saved { .. })
    ));
    assert_eq!(
        view.borrow()
            .as_ref()
            .map(|snapshot| snapshot.logbooks.selected_view),
        Some(SelectedView::AllContacts)
    );
    let selection = SettingsAction::Logbooks(LogbookAction::SelectView {
        next: SelectedView::Logbook(tag.id),
    });
    assert!(
        matches!(request(&sender, selection).await?, SettingsResponse::Logbooks(LogbookResponse::Selected(view)) if view == SelectedView::Logbook(tag.id))
    );
    let SettingsResponse::Logbooks(LogbookResponse::DeletionPreview(preview)) = request(
        &sender,
        SettingsAction::Logbooks(LogbookAction::PreviewDeletion { id: tag.id }),
    )
    .await?
    else {
        anyhow::bail!("the live tag should return a deletion preview");
    };
    assert_eq!(preview.counts.affected, 0);
    let mut changes = AccountChanges::default();
    changes
        .tag_deletions
        .push(changes.review_tag(&preview, TagDeletionChoice::LeaveAll)?);
    let before =
        fixture.settings_basis.borrow().clone().ok_or_else(|| {
            anyhow::anyhow!("the open editor should have its selected logbook basis")
        })?;
    assert_eq!(
        before.snapshot.logbooks.selected_view,
        SelectedView::Logbook(tag.id)
    );
    let SettingsResponse::ApplyAccountChanges(deleted) =
        request(&sender, SettingsAction::ApplyAccountChanges(changes)).await?
    else {
        anyhow::bail!("the reviewed deletion should return the account operation results")
    };
    let Some(AccountOutcome::DeletedLogbook {
        plan,
        fallback_from,
    }) = deleted.outcomes.get(&RecordId::Tag(tag.id))
    else {
        anyhow::bail!("deleting the selected empty logbook should return its effects")
    };
    assert!(plan.affected.is_empty());
    assert_eq!(*fallback_from, Some(tag.id));
    assert_eq!(
        view.borrow()
            .as_ref()
            .map(|snapshot| snapshot.logbooks.selected_view),
        Some(SelectedView::AllContacts)
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
            .iter()
            .find(|row| row.key().id == RecordId::Tag(tag.id))
            .and_then(clacks_sync::client::ClientRecord::working_state),
        Some(clacks_sync::record::RecordState::Deleted)
    ));
    reopened.store.close().await?;

    // A failed follow-up read must publish this same acknowledged deletion to
    // both consumers before the user closes Settings or the recovery read runs.
    let (published, workbench) = watch::channel(Some(before.snapshot.clone()));
    let (ready, editor) = watch::channel(Some(before));
    let (reply, outcome) = oneshot::channel();
    let completion = session::settings::complete(
        reply,
        SettingsResponse::ApplyAccountChanges(deleted),
        &reopened.store,
        "apply_account_changes",
    )
    .await;
    let mut recovery = session::settings_view::Recovery::default();
    session::settle_settings(
        Ok(Some(completion)),
        &published,
        &ready,
        &Default::default(),
        &mut recovery,
    );
    assert!(matches!(
        outcome.await?,
        SettingsResponse::RefreshFailed { .. }
    ));
    assert!(recovery.scheduled());
    let workbench = workbench.borrow();
    let editor = editor.borrow();
    let workbench = workbench
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("workbench snapshot missing"))?;
    let editor = editor
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("editor basis missing"))?;
    assert_eq!(workbench.logbooks, reopened.settings.logbooks);
    assert!(std::sync::Arc::ptr_eq(workbench, &editor.snapshot));
    Ok(())
}
