//! Concrete selected-directory material; never part of replicated record history.

use clacks_model::ids::DirectoryMaterialId;
use clacks_model::{directory::values::DirectoryConfiguration, ids::AccountId};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Login bytes erased when released and deliberately redacted from diagnostics.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct DirectoryLogin {
    /// Provider account name, sent only to its selected authenticated directory.
    pub username: String,
    /// Exact provider password, never an empty-field removal instruction.
    pub password: String,
}

impl std::fmt::Debug for DirectoryLogin {
    /// Format only the type; both account name and password remain private.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectoryLogin").finish_non_exhaustive()
    }
}

/// Complete account material identity and revision authenticated by its owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryMaterial {
    /// Stable identity until removal; replacement increments its revision.
    pub id: DirectoryMaterialId,
    /// Positive material version; encryption maintenance does not advance it.
    pub revision: i64,
    /// Sensitive material whose diagnostic implementation excludes its contents.
    pub login: DirectoryLogin,
}

/// Current account-authorized selected material; never a replicated record value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedConfiguration {
    /// Owner verified by the server's native resource authorization.
    pub account: AccountId,
    /// Public selection and material availability read consistently with the login.
    pub configuration: DirectoryConfiguration,
    /// Selected material only, absent when disabled or unconfigured.
    pub material: Option<DirectoryMaterial>,
}

impl ProvisionedConfiguration {
    /// Reject inconsistent selection/material before native persistence.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.configuration.revision >= 0,
            "invalid provisioned configuration revision"
        );
        let available = self
            .configuration
            .selected
            .is_some_and(|provider| self.configuration.configured.contains(&provider));
        anyhow::ensure!(
            available == self.material.is_some(),
            "provisioned material differs from selected availability"
        );
        if let Some(material) = &self.material {
            anyhow::ensure!(
                material.revision > 0
                    && !material.login.username.trim().is_empty()
                    && !material.login.password.is_empty(),
                "invalid provisioned directory material"
            );
        }
        Ok(())
    }
}
