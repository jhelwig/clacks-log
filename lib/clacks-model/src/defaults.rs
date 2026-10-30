//! Bundled, ordinary native definitions for initializing a new account.

use crate::{
    export::{OutputProfile, OutputPurpose, rules::compile_definition},
    group::{DefinitionSet, GroupDefinition},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Initial account-shared groups and output profiles, separate from recorded contacts.
///
/// Install these only when provisioning a new account. Existing account definitions
/// remain authoritative when reopening a workspace or upgrading the application.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountDefinitions {
    /// Reference/participation groups interpreted by the ordinary definition engine.
    pub groups: Vec<GroupDefinition>,
    /// Export contexts addressed by the groups' targeted mappings.
    pub output_profiles: Vec<OutputProfile>,
}

impl AccountDefinitions {
    /// Check identity uniqueness, mapping validity and targeted profile references.
    ///
    /// Call before installing supplied initial definitions. Errors identify unusable
    /// definitions; validation does not normalize references or create account records.
    pub fn validate(&self) -> anyhow::Result<()> {
        let definitions = DefinitionSet::new(self.groups.clone())?;
        anyhow::ensure!(
            definitions.iter().count() == self.groups.len(),
            "duplicate initial group ID"
        );
        let mut profiles = BTreeMap::new();

        for profile in &self.output_profiles {
            anyhow::ensure!(
                !profile.name.trim().is_empty(),
                "blank initial output profile name"
            );
            anyhow::ensure!(
                profiles.insert(profile.id, profile).is_none(),
                "duplicate initial output profile ID"
            );
        }

        for group in &self.groups {
            compile_definition(group)?;
            for target in group.export_mappings().targeted.keys() {
                let profile = profiles.get(target).ok_or_else(|| {
                    anyhow::anyhow!(
                        "group {} targets missing output profile {target}",
                        group.id()
                    )
                })?;
                anyhow::ensure!(
                    profile.purpose == OutputPurpose::Targeted,
                    "group {} targets an ordinary output profile",
                    group.id()
                );
            }
        }

        Ok(())
    }
}

/// Load the bundled seed data as validated native values for a new account.
///
/// This reads embedded data without filesystem/network access. It does not install
/// definitions, create accounts, overwrite saved values or allocate fresh identities.
pub fn account_definitions() -> anyhow::Result<AccountDefinitions> {
    let definitions: AccountDefinitions =
        serde_json::from_str(include_str!("../data/account-definitions.json"))?;
    definitions.validate()?;
    Ok(definitions)
}
