//! Typed grant lifecycle restored before authorization decisions.

use clacks_model::ids::ClientActorId;
use jiff::Timestamp;

/// Coherent admission state of an account-owned OAuth grant family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GrantState {
    /// A bounded, not-yet-activated approval.
    Provisional {
        /// Last instant at which the original approval may activate.
        until: Timestamp,
    },
    /// A grant attached to an enrolled actor whose current admission is checked separately.
    Active {
        /// Actor associated with this active family.
        actor: ClientActorId,
    },
    /// Explicitly invalidated grant.
    Revoked,
}

impl GrantState {
    /// Restore the lifecycle alternative from raw database columns.
    pub(crate) fn from_columns(
        state: &str,
        actor: Option<&str>,
        until: Option<Timestamp>,
    ) -> anyhow::Result<Self> {
        let actor = actor.map(str::parse).transpose()?;
        match (state, actor, until) {
            ("provisional", None, Some(until)) => Ok(Self::Provisional { until }),
            ("active", Some(actor), None) => Ok(Self::Active { actor }),
            ("revoked", None, Some(_)) => Ok(Self::Revoked),
            ("revoked", Some(_), None) => Ok(Self::Revoked),
            ("provisional", _, _) => {
                anyhow::bail!("provisional grant requires a deadline and no actor")
            }
            ("active", _, _) => {
                anyhow::bail!("active grant requires an actor and no provisional deadline")
            }
            ("revoked", _, _) => {
                anyhow::bail!("revoked grant requires exactly one retained admission origin")
            }
            _ => anyhow::bail!("unknown grant lifecycle discriminator"),
        }
    }

    /// Return the active grant actor when the supplied membership closure is absent.
    pub(crate) fn active_actor(self, closed_at: Option<Timestamp>) -> Option<ClientActorId> {
        match self {
            Self::Active { actor } if closed_at.is_none() => Some(actor),
            _ => None,
        }
    }
}
