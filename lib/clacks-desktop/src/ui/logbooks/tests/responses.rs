use super::*;

#[test]
fn deleted_view_fallback_is_reported_once_with_safe_context() -> anyhow::Result<()> {
    let tag = LogbookTag::new(Default::default(), "Private logbook name".into());
    let record = fixtures::row(RecordValue::Tag(Box::new(tag.clone())));
    let account = record.key().account;
    let before = LogbookSnapshot {
        selected_view: SelectedView::Logbook(tag.id),
        tags: vec![record.clone()],
    };
    let mut fields = record.into_fields();
    fields.pending.as_mut().unwrap().working = RecordState::Deleted;
    let deleted: clacks_sync::client::ClientRecord = fields.try_into().unwrap();
    let after = LogbookSnapshot {
        selected_view: SelectedView::AllContacts,
        tags: vec![deleted],
    };
    let output = tempfile::NamedTempFile::new()?;
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::sync::Mutex::new(output.reopen()?))
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let mut view = LogbooksUi::default();
        view.observe(&before, None);
        view.observe(&after, None);
        view.fallback_notice = None;
        view.observe(&after, None);
    });
    let logged = std::fs::read_to_string(output.path())?;
    assert_eq!(
        logged
            .lines()
            .filter(|line| line.contains("logbook.view_fallback_observed"))
            .count(),
        1
    );
    assert!(logged.contains(&account.to_string()));
    assert!(logged.contains(&tag.id.to_string()));
    assert!(logged.contains("deleted_tag"));
    assert!(!logged.contains(&tag.name));
    Ok(())
}
