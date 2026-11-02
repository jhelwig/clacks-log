//! Selected issuer diagnostics through oauth-as's standard event hook.

use oauth_as::events::{Event, EventSink};

/// Reports issuer outcomes without serializing credentials or untrusted request payloads.
pub(super) struct IssuerEvents;

impl EventSink for IssuerEvents {
    fn on_event(&self, event: Event<'_>) {
        match event {
            Event::TokenIssued {
                grant_type,
                subject,
                family_id,
                ..
            } => {
                metrics::counter!("clacks_oauth_tokens_issued_total", "grant_type" => grant_type.as_str()).increment(1);
                tracing::info!(
                    event = "oauth.token.issued",
                    ?grant_type,
                    account_id = subject,
                    grant_family = family_id,
                    "Bound token issued"
                );
            }
            Event::GrantRefused {
                grant_type, error, ..
            } => {
                tracing::debug!(
                    event = "oauth.grant.refused",
                    ?grant_type,
                    ?error,
                    "Grant request refused"
                );
            }
            Event::DpopProofRefused { failure } => {
                tracing::debug!(
                    event = "oauth.proof.refused",
                    ?failure,
                    "Issuer proof refused"
                );
            }
            Event::ClientAuthenticationFailed { failure, .. } => {
                tracing::debug!(
                    event = "oauth.client.refused",
                    ?failure,
                    "Public client authentication refused"
                );
            }
            Event::AuthorizationCodeReplayDetected {
                tokens_revoked,
                containment_failed,
                family_id,
                ..
            } => {
                if containment_failed {
                    tracing::error!(
                        event = "oauth.code.replay",
                        grant_family = family_id,
                        tokens_revoked,
                        containment_failed,
                        "Code replay containment did not persist"
                    );
                } else {
                    tracing::warn!(
                        event = "oauth.code.replay",
                        grant_family = family_id,
                        tokens_revoked,
                        containment_failed,
                        "Authorization code replay refused"
                    );
                }
            }
            Event::TokenRevoked {
                token_type,
                cascade_failed,
                ..
            } => {
                if cascade_failed {
                    tracing::error!(
                        event = "oauth.token.revoked",
                        ?token_type,
                        cascade_failed,
                        "Token revoked but grant invalidation failed"
                    );
                } else {
                    tracing::info!(
                        event = "oauth.token.revoked",
                        ?token_type,
                        "Token authorization revoked"
                    );
                }
            }
            // Disabled grants and registration facilities produce no Clacks events.
            _ => {}
        }
    }
}
