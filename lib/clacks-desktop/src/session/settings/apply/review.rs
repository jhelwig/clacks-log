//! Project destruction in confirmation order and resolve exact transaction guards at Apply.

use super::*;
use clacks_model::tag::plan_tag_deletion;

impl AccountChanges {
    /// Project earlier approved QSO changes onto this tag's original deletion preview.
    pub fn review_tag(
        &self,
        preview: &Arc<TagDeletionPreview>,
        choice: TagDeletionChoice,
    ) -> anyhow::Result<TagReview> {
        let RecordId::Tag(id) = preview.tag.key().id else {
            anyhow::bail!("The deletion preview requires a logbook.")
        };
        let mut projected = BTreeMap::new();
        let mut qsos = Vec::with_capacity(preview.members.len());
        for original in &preview.members {
            let Some(RecordState::Live(RecordValue::Qso(qso))) = original.working_state() else {
                anyhow::bail!("A reviewed contact is unavailable.")
            };
            let mut tags = qso.tags().clone();
            let mut deleted = false;
            for prior in &self.tag_deletions {
                if prior.plan.mark_deleted.contains(&qso.id()) {
                    deleted = true;
                    break;
                }
                if prior.plan.remove_membership.contains(&qso.id()) {
                    tags.remove(&prior.id);
                }
            }
            if deleted || !tags.contains(&id) {
                continue;
            }
            if &tags == qso.tags() {
                qsos.push((**qso).clone());
            } else {
                let mut fields = (**qso).clone().into_fields();
                fields.tags = tags;
                let next = Qso::try_from(fields)?;
                projected.insert(next.id(), RecordValue::Qso(Box::new(next.clone())));
                qsos.push(next);
            }
        }
        let plan = plan_tag_deletion(id, choice, &qsos);
        let counts = TagDeletionCounts {
            affected: plan.affected.len(),
            also_tagged_elsewhere: plan.also_tagged_elsewhere,
            unique: plan.affected.len() - plan.also_tagged_elsewhere,
        };
        Ok(TagReview {
            id,
            counts,
            choice,
            plan,
            basis: preview.clone(),
            projected,
        })
    }

    /// Resolve the live station profiles after staged saves, removals and confirmed repairs.
    pub fn projected_profiles(
        &self,
        snapshot: &SettingsSnapshot,
    ) -> anyhow::Result<BTreeMap<ProfileId, SavedLocationProfile>> {
        let mut profiles: BTreeMap<_, _> = snapshot
            .profiles
            .iter()
            .filter_map(|r| {
                let Some(RecordState::Live(RecordValue::Profile(value))) = r.working_state() else {
                    return None;
                };
                Some((value.id(), (**value).clone()))
            })
            .collect();
        profiles.extend(
            self.profiles
                .iter()
                .map(|(id, edit)| (*id, edit.next.clone())),
        );
        for id in self.deleted_profiles.keys() {
            profiles.remove(id);
        }
        for review in &self.equipment_deletions {
            for choice in &review.replacements {
                if let Some(profile) = profiles.get_mut(&choice.profile)
                    && profile.default_equipment() == Some(review.id)
                {
                    let mut fields = profile.clone().into_fields();
                    fields.default_equipment = choice.equipment;
                    *profile = fields.try_into()?;
                }
            }
        }
        Ok(profiles)
    }

    /// Capture the target, projected referring profiles and live replacements shown for review.
    pub fn review_equipment(
        &self,
        snapshot: &SettingsSnapshot,
        id: EquipmentConfigurationId,
    ) -> anyhow::Result<EquipmentReview> {
        let target = snapshot
            .equipment
            .iter()
            .find(|r| {
                r.key().id == RecordId::Equipment(id)
                    && matches!(
                        r.working_state(),
                        Some(RecordState::Live(RecordValue::Equipment(_)))
                    )
            })
            .ok_or_else(|| anyhow::anyhow!("The selected equipment is unavailable."))?
            .clone();
        let profiles = self
            .projected_profiles(snapshot)?
            .into_iter()
            .filter(|(_, p)| p.default_equipment() == Some(id))
            .collect();
        let mut alternatives: BTreeMap<_, _> = snapshot
            .equipment
            .iter()
            .filter_map(|r| {
                let Some(RecordState::Live(RecordValue::Equipment(value))) = r.working_state()
                else {
                    return None;
                };
                Some((value.id(), (**value).clone()))
            })
            .collect();
        alternatives.extend(
            self.equipment
                .iter()
                .map(|(id, edit)| (*id, edit.next.clone())),
        );
        alternatives.remove(&id);
        for prior in &self.equipment_deletions {
            alternatives.remove(&prior.id);
        }
        let mut bases: BTreeMap<_, _> = snapshot
            .equipment
            .iter()
            .chain(&snapshot.profiles)
            .map(|r| (r.key().id, r.clone()))
            .collect();
        for (key, basis) in self
            .equipment
            .iter()
            .map(|(id, edit)| (RecordId::Equipment(*id), &edit.expected))
            .chain(
                self.profiles
                    .iter()
                    .map(|(id, edit)| (RecordId::Profile(*id), &edit.expected)),
            )
        {
            match basis {
                Some(row) => {
                    bases.insert(key, row.clone());
                }
                None => {
                    bases.remove(&key);
                }
            }
        }
        Ok(EquipmentReview {
            id,
            profiles,
            alternatives,
            replacements: Vec::new(),
            target,
            bases,
        })
    }
}

/// Diagnostic for an impact which differs from the operator's confirmation.
fn stale() -> AccountOutcome {
    AccountOutcome::Rejected("The deletion impact changed. Review it again before deleting.".into())
}

impl TagReview {
    /// Resolve the reviewed QSO states against earlier acknowledgments, then recheck inside the deletion transaction.
    pub(super) async fn apply(
        &self,
        store: &ClientStore,
        result: &mut AccountApplyResult,
    ) -> AccountOutcome {
        let mut members = Vec::with_capacity(self.plan.affected.len());
        for basis in &self.basis.members {
            let RecordId::Qso(id) = basis.key().id else {
                return stale();
            };
            if self.plan.affected.contains(&id) {
                let Some(RecordState::Live(original)) = basis.working_state() else {
                    return stale();
                };
                let value = self.projected.get(&id).unwrap_or(original);
                let Some(row) = result.reviewed(basis.key().id, Some(basis), value) else {
                    return stale();
                };
                members.push(row);
            } else {
                // This member was excluded by an earlier confirmed deletion.
                let Some(change) = result.records.get(&basis.key().id) else {
                    return stale();
                };
                if change.original.as_ref() != Some(basis)
                    || !matches!(change.current.working_state(), Some(RecordState::Deleted))
                {
                    return stale();
                }
            }
        }
        let expected = TagDeletionPreview {
            tag: self.basis.tag.clone(),
            members,
            counts: self.counts.clone(),
        };
        match deletion::delete_tag(store, &expected, self.choice).await {
            Ok(TagDeleteOutcome::Applied {
                plan,
                records,
                selected_view,
                fallback_from,
            }) => {
                let bases: BTreeMap<_, _> = expected
                    .members
                    .iter()
                    .chain(std::iter::once(&expected.tag))
                    .map(|r| (r.key().id, r))
                    .collect();
                for row in records {
                    result.acknowledge(bases.get(&row.key().id).copied(), row);
                }
                result.selected_view = Some(selected_view);
                AccountOutcome::DeletedLogbook {
                    plan,
                    fallback_from,
                }
            }
            Ok(TagDeleteOutcome::Stale { current }) => {
                result
                    .observed
                    .insert(RecordId::Tag(self.id), current.map(|r| *r));
                stale()
            }
            Err(error) => AccountOutcome::Unverified(format!("{error:#}")),
        }
    }
}

impl EquipmentReview {
    /// Resolve each projected referrer and selected replacement to its exact guarded record.
    fn preview(&self, result: &AccountApplyResult) -> Option<EquipmentDeletionPreview> {
        let mut referring_profiles = Vec::with_capacity(self.profiles.len());
        for (id, profile) in &self.profiles {
            let key = RecordId::Profile(*id);
            referring_profiles.push(result.reviewed(
                key,
                self.bases.get(&key),
                &RecordValue::Profile(Box::new(profile.clone())),
            )?);
        }
        let mut replacement_equipment = BTreeMap::new();
        for id in self
            .replacements
            .iter()
            .filter_map(|choice| choice.equipment)
        {
            let alternative = self.alternatives.get(&id)?;
            let key = RecordId::Equipment(id);
            replacement_equipment.insert(
                id,
                result.reviewed(
                    key,
                    self.bases.get(&key),
                    &RecordValue::Equipment(Box::new(alternative.clone())),
                )?,
            );
        }
        Some(EquipmentDeletionPreview {
            equipment: self.target.clone(),
            referring_profiles,
            replacement_equipment: replacement_equipment.into_values().collect(),
        })
    }

    /// Apply the projected review through the store's atomic target, referrer and replacement checks.
    pub(super) async fn apply(
        &self,
        store: &ClientStore,
        result: &mut AccountApplyResult,
    ) -> AccountOutcome {
        let Some(expected) = self.preview(result) else {
            return stale();
        };
        match settings::delete_equipment(store, &expected, &self.replacements).await {
            Ok(EquipmentDeleteOutcome::Applied { records }) => {
                for row in records {
                    let basis = self.bases.get(&row.key().id);
                    result.acknowledge(basis, row);
                }
                AccountOutcome::Saved {
                    missing_equipment: None,
                }
            }
            Ok(EquipmentDeleteOutcome::Stale(preview)) => {
                for row in std::iter::once(preview.equipment)
                    .chain(preview.referring_profiles)
                    .chain(preview.replacement_equipment)
                {
                    result.observed.insert(row.key().id, Some(row));
                }
                stale()
            }
            Ok(EquipmentDeleteOutcome::Unavailable { current }) => {
                result
                    .observed
                    .insert(RecordId::Equipment(self.id), current);
                stale()
            }
            Err(error) => AccountOutcome::Unverified(format!("{error:#}")),
        }
    }
}
