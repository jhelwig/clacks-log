//! User-safe connection failures and typed recovery decisions.

use super::EnrollmentStatus;

/// Application-owned boundaries whose explanations contain no remote payloads.
#[derive(Debug, thiserror::Error)]
pub(super) enum Problem {
    /// The selected origin cannot replace an installed account connection.
    #[error("This workspace must connect to its existing server.")]
    Server,
    /// A fresh browser approval selected a different account.
    #[error("Sign in with this workspace's existing account.")]
    Account,
    /// A native operation failed without establishing credential absence.
    #[error(
        "Could not access this app's saved credentials. Check credential-store access and try again."
    )]
    Credentials,
    /// The authorization or refresh exchange could not complete.
    #[error("The server could not complete sign-in. Try connecting again.")]
    Authorization,
    /// Connection committed but local bookkeeping remains incomplete.
    #[error("Connected, but local cleanup could not finish. Try again to finish setup.")]
    Cleanup,
    /// Persistence or a saved-state guard failed.
    #[error("Could not finish connection setup. Your existing data has been preserved. Try again.")]
    Storage,
    /// A form value failed application validation.
    #[error("Check the server address and enter a client label of 1–128 characters.")]
    Input,
}

/// Only a server verdict on the refresh credential justifies fresh authorization.
pub(super) fn refresh_rejected(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<huskarl::core::Error>())
        .any(|error| {
            error.verdict().is_some_and(|verdict| {
                verdict.code() == &huskarl::core::OAuthErrorCode::InvalidGrant
            })
        })
}

/// Missing native entries are recoverable; denied or unreadable entries are not absence.
pub(super) fn missing(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<keyring_core::Error>(),
        Some(keyring_core::Error::NoEntry)
    )
}

/// Present only application-authored summaries and bounded diagnostic categories.
pub(super) fn present(error: &anyhow::Error) -> EnrollmentStatus {
    let problem = error.downcast_ref::<Problem>();
    let message = if matches!(
        error.downcast_ref::<clacks_client_store::error::StoreError>(),
        Some(clacks_client_store::error::StoreError::PopulatedWorkspace)
    ) {
        "This log contains local data and cannot be connected automatically. Export your contacts, connect an empty log, then import them.".into()
    } else if matches!(problem, Some(Problem::Cleanup)) {
        Problem::Cleanup.to_string()
    } else if error.downcast_ref::<keyring_core::Error>().is_some() {
        Problem::Credentials.to_string()
    } else if let Some(problem) = problem {
        problem.to_string()
    } else {
        Problem::Storage.to_string()
    };
    let details = if error.downcast_ref::<keyring_core::Error>().is_some() {
        "Native credential storage could not complete the operation.".into()
    } else if let Some(status) = error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<reqwest_oauth::Error>())
        .find_map(|error| error.status())
    {
        format!("The server returned HTTP {}.", status.as_u16())
    } else if error
        .chain()
        .any(|cause| cause.is::<huskarl::core::Error>())
    {
        "The authorization exchange did not complete.".into()
    } else {
        format!(
            "Failure category: {}.",
            crate::telemetry::failure::error_class(error)
        )
    };
    EnrollmentStatus::Failed {
        enrolled: false,
        message,
        details: Some(details),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prevent a transient server failure from replacing credentials, and prevent
    /// arbitrary server descriptions from entering diagnostic UI payloads.
    #[test]
    fn recovery_uses_verdict_and_never_renders_remote_error_payloads() {
        for (status, code, restart) in [
            (400, "invalid_grant", true),
            (500, "invalid_grant", false),
            (429, "invalid_grant", false),
            (400, "invalid_client", false),
        ] {
            let headers = http::HeaderMap::new();
            let error: anyhow::Error = huskarl::core::http::FailedResponse::new(
                http::StatusCode::from_u16(status).unwrap(),
                &headers,
            )
            .unwrap()
            .into_error(
                Some(
                    huskarl::core::OAuthError::new(code)
                        .with_description(Some("PRIVATE_TOKEN".into())),
                ),
                "PRIVATE_BODY",
            )
            .into();
            let error = error.context(Problem::Authorization);
            assert_eq!(refresh_rejected(&error), restart);
            let shown = format!("{:?}", present(&error));
            assert!(!shown.contains("PRIVATE_TOKEN"));
            assert!(!shown.contains("PRIVATE_BODY"));
        }
        let denied: anyhow::Error = keyring_core::Error::PlatformFailure(Box::new(
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        ))
        .into();
        let denied = denied.context(Problem::Authorization);
        assert!(!missing(&denied));
        assert!(!refresh_rejected(&denied));
        assert!(
            matches!(present(&denied), EnrollmentStatus::Failed { message, .. } if message.contains("saved credentials"))
        );
    }
}
