//! Typed settings requests on the existing application runtime.

/// Metric name for completed contact read including snapshot release.
pub(crate) const CONTACT_QUERY_DURATION_METRIC: &str =
    "client_store.contact_query.duration_seconds";

#[cfg(test)]
mod tests;

pub(crate) mod contacts;
pub(crate) mod logbooks;
pub(crate) mod review;

use clacks_client_store::{
    operating::{self, OperatingSaveOutcome, OperatingState},
    save::SaveOutcome,
    settings::{
        self, EquipmentDeleteOutcome, EquipmentDeletionPreview, ProfileEquipmentReplacement,
        ProfileSaveResult, SettingsSnapshot,
    },
    store::ClientStore,
};
use clacks_model::{
    ids::EquipmentConfigurationId,
    profile::{EquipmentConfiguration, SavedLocationProfile},
};
use clacks_sync::client::ClientRecord;
use tokio::sync::oneshot;

/// An owned application operation, independent of the lifecycle command channel.
pub(crate) enum SettingsAction {
    /// Read account-wide pending work and actual comparisons.
    Review(review::ReviewAction),
    /// Read or edit account-owned contact working state.
    Contacts(contacts::ContactAction),
    /// Manage logbooks or the local view without altering operating settings.
    Logbooks(logbooks::LogbookAction),
    /// Reload the current committed settings view.
    Refresh,
    /// Commit a validated operating edit under its expected active basis.
    Apply {
        /// Applied state read when the operating editor opened.
        expected: OperatingState,
        /// Complete desired facts and optional source selections.
        next: OperatingState,
    },
    /// Save a profile as shared local working state, without applying it.
    SaveProfile {
        /// Existing target basis, absent for a new profile.
        expected: Option<ClientRecord>,
        /// Profile-owned values to save independently.
        next: SavedLocationProfile,
    },
    /// Save reusable equipment without selecting it for operation.
    SaveEquipment {
        /// Existing target basis, absent for a new configuration.
        expected: Option<ClientRecord>,
        /// Configuration-owned values to save independently.
        next: EquipmentConfiguration,
    },
    /// Delete a profile and detach its active selection if needed.
    DeleteProfile {
        /// Profile version shown when deletion was requested.
        expected: ClientRecord,
    },
    /// Read the full equipment-reference review before a user decides.
    PreviewEquipmentDeletion {
        /// Account-local configuration identity to review.
        id: EquipmentConfigurationId,
    },
    /// Commit an explicit complete reference-repair review.
    DeleteEquipment {
        /// Exact equipment, referrers and candidate replacements reviewed together.
        expected: EquipmentDeletionPreview,
        /// Explicit replacement or removal for every referring profile.
        replacements: Vec<ProfileEquipmentReplacement>,
    },
}

/// One request with a response receiver owned by its originating editor.
pub(crate) struct SettingsRequest {
    /// Owned operation inputs.
    pub action: SettingsAction,
    /// Exactly one result, independent of later UI navigation.
    pub reply: oneshot::Sender<SettingsResponse>,
}

/// Authoritative operation outcome delivered independently from view refresh.
#[derive(Debug)]
pub(crate) enum SettingsResponse {
    /// Snapshot result for the originating review view.
    Review(review::ReviewResponse),
    /// Authoritative contact result.
    Contacts(contacts::ContactResponse),
    /// Authoritative outcome from the originating logbook workflow.
    Logbooks(logbooks::LogbookResponse),
    /// Refreshed committed state.
    Refresh(SettingsSnapshot),
    /// Applied, unchanged or stale operating state.
    Apply(OperatingSaveOutcome),
    /// Profile save and its optional missing-reference issue.
    SaveProfile(ProfileSaveResult),
    /// Saved, unchanged or stale equipment record.
    SaveEquipment(SaveOutcome),
    /// Profile deletion outcome.
    DeleteProfile(SaveOutcome),
    /// Exact review inputs for equipment deletion.
    PreviewEquipmentDeletion(EquipmentDeletionPreview),
    /// Committed deletion or a refreshed stale review.
    DeleteEquipment(EquipmentDeleteOutcome),
    /// The operation failed without a successful commit acknowledgment.
    Failed(String),
    /// The operation outcome remains authoritative despite a failed subsequent refresh.
    RefreshFailed {
        /// Authoritative operation result, including any successful commit.
        outcome: Box<SettingsResponse>,
        /// User-visible cause of the failed following view read.
        message: String,
    },
    /// Admission stopped before this request began.
    Closing,
}

/// A settled request and its consistent view, ready for ordered window publication.
pub(super) struct Completion {
    /// The operation's own response channel.
    pub reply: oneshot::Sender<SettingsResponse>,
    /// Native result retained even when refreshing the view fails.
    pub response: SettingsResponse,
    /// Fresh committed state, absent on failed operations or refreshes.
    pub snapshot: Option<SettingsSnapshot>,
}

/// Execute admitted domain work off the native UI thread and retain committed outcomes.
#[tracing::instrument(skip_all, name = "desktop.settings")]
pub(super) async fn execute(
    store: ClientStore,
    mut request: SettingsRequest,
    lookup: Option<crate::directory::lookup::LookupClient>,
    changes: tokio::sync::watch::Sender<()>,
) -> Option<Completion> {
    let account = store.account_id();
    let action_kind = request.action.diagnostic_name();

    let result = match request.action {
        SettingsAction::Review(action) => review::execute(&store, action)
            .await
            .map(SettingsResponse::Review),
        SettingsAction::Contacts(action) => {
            let kind = action.query_kind();
            let preparation = matches!(&action, contacts::ContactAction::PrepareMembership { .. });
            let result = if let Some(kind) = kind {
                let started = std::time::Instant::now();
                let result = tokio::select! {
                    biased;
                    _ = request.reply.closed() => {
                        tracing::debug!(event = "contact_query.cancelled", account_id = %account, kind);
                        return None;
                    }
                    result = contacts::execute(&store, action, lookup, changes) => result,
                };
                metrics::histogram!(CONTACT_QUERY_DURATION_METRIC, "kind" => kind)
                    .record(started.elapsed().as_secs_f64());
                match &result {
                    Ok(
                        contacts::ContactResponse::Page(page)
                        | contacts::ContactResponse::WorkedBefore(page),
                    ) => {
                        tracing::debug!(event = "contact_query.completed", account_id = %account, kind, matching_count = page.matching_total, returned_count = page.rows.len());
                    }
                    Ok(contacts::ContactResponse::SelectedAll(ids)) => {
                        tracing::debug!(event = "contact_query.completed", account_id = %account, kind, returned_count = ids.len());
                    }
                    Err(_) => {
                        metrics::counter!("client_store.contact_query.failures_total", "kind" => kind).increment(1);
                    }
                    _ => {}
                }
                result
            } else if preparation {
                tokio::select! {
                    _ = request.reply.closed() => return None,
                    result = contacts::execute(&store, action, lookup, changes) => result,
                }
            } else {
                contacts::execute(&store, action, lookup, changes).await
            };
            result.map(SettingsResponse::Contacts)
        }
        SettingsAction::Logbooks(action) => logbooks::execute(&store, action)
            .await
            .map(SettingsResponse::Logbooks),
        SettingsAction::Refresh => settings::snapshot(&store)
            .await
            .map(SettingsResponse::Refresh),
        SettingsAction::Apply { expected, next } => operating::save(&store, &expected, &next)
            .await
            .map(SettingsResponse::Apply),
        SettingsAction::SaveProfile { expected, next } => {
            settings::save_profile(&store, expected.as_ref(), &next)
                .await
                .map(SettingsResponse::SaveProfile)
        }
        SettingsAction::SaveEquipment { expected, next } => {
            settings::save_equipment(&store, expected.as_ref(), &next)
                .await
                .map(SettingsResponse::SaveEquipment)
        }
        SettingsAction::DeleteProfile { expected } => settings::delete_profile(&store, &expected)
            .await
            .map(SettingsResponse::DeleteProfile),
        SettingsAction::PreviewEquipmentDeletion { id } => {
            settings::preview_equipment_deletion(&store, id)
                .await
                .map(SettingsResponse::PreviewEquipmentDeletion)
        }
        SettingsAction::DeleteEquipment {
            expected,
            replacements,
        } => settings::delete_equipment(&store, &expected, &replacements)
            .await
            .map(SettingsResponse::DeleteEquipment),
    };
    let mut response = match result {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(event = "desktop.settings_failed", account_id = %account, action = %action_kind, error.type = crate::telemetry::failure::error_class(&error));
            return Some(Completion {
                reply: request.reply,
                response: SettingsResponse::Failed(format!("{error:#}")),
                snapshot: None,
            });
        }
    };
    let snapshot = match &response {
        SettingsResponse::Review(
            review::ReviewResponse::Page { .. }
            | review::ReviewResponse::Record { .. }
            | review::ReviewResponse::References { .. },
        ) => None,
        SettingsResponse::Contacts(
            contacts::ContactResponse::Page(_)
            | contacts::ContactResponse::DirectoryLookupStarted(_)
            | contacts::ContactResponse::SelectedAll(_)
            | contacts::ContactResponse::WorkedBefore(_)
            | contacts::ContactResponse::MembershipPrepared(_)
            | contacts::ContactResponse::Loaded(_, _)
            | contacts::ContactResponse::QueryState(_)
            | contacts::ContactResponse::QueryStateSaved(_),
        ) => None,
        SettingsResponse::Refresh(view) => Some(view.clone()),
        _ => match settings::snapshot(&store).await {
            Ok(view) => Some(view),
            Err(error) => {
                tracing::error!(event = "desktop.settings_refresh_failed", account_id = %account, action = %action_kind, error.type = crate::telemetry::failure::error_class(&error));
                response = SettingsResponse::RefreshFailed {
                    outcome: Box::new(response),
                    message: format!(
                        "The operation finished, but refreshing settings failed: {error:#}"
                    ),
                };
                None
            }
        },
    };
    Some(Completion {
        reply: request.reply,
        response,
        snapshot,
    })
}

impl SettingsAction {
    /// Compose the containing action with its local operation name, without payloads.
    pub(crate) fn diagnostic_name(&self) -> std::borrow::Cow<'static, str> {
        match self {
            Self::Review(action) => format!("review.{}", action.diagnostic_name()).into(),
            Self::Contacts(action) => format!("contacts.{}", action.diagnostic_name()).into(),
            Self::Logbooks(action) => format!("logbooks.{}", action.diagnostic_name()).into(),
            Self::Refresh => "refresh".into(),
            Self::Apply { .. } => "apply_operating".into(),
            Self::SaveProfile { .. } => "save_profile".into(),
            Self::SaveEquipment { .. } => "save_equipment".into(),
            Self::DeleteProfile { .. } => "delete_profile".into(),
            Self::PreviewEquipmentDeletion { .. } => "preview_equipment_deletion".into(),
            Self::DeleteEquipment { .. } => "delete_equipment".into(),
        }
    }

    /// Identify account data writes whose settled outcomes should wake native synchronization.
    pub(crate) fn is_shared_write(&self) -> bool {
        matches!(
            self,
            Self::Review(
                review::ReviewAction::Resolve { .. } | review::ReviewAction::RetryDeletion { .. }
            ) | Self::Contacts(
                contacts::ContactAction::Save { .. }
                    | contacts::ContactAction::Delete { .. }
                    | contacts::ContactAction::ChangeMembership(_)
            ) | Self::Logbooks(
                logbooks::LogbookAction::SaveTag { .. } | logbooks::LogbookAction::DeleteTag { .. }
            ) | Self::SaveProfile { .. }
                | Self::SaveEquipment { .. }
                | Self::DeleteProfile { .. }
                | Self::DeleteEquipment { .. }
        )
    }
}
