//! Client-owned review of accepted facts that cannot be installed automatically by the hub.

use super::{ClientRecord, ClientUpdate, PendingRecord, ReviewInput};
use crate::record::Observation;

/// Preserve a returned comparison and its reason without discarding newer local work.
///
/// Use this for an authenticated recovery review, rather than treating its accepted
/// alternatives as ordinary automatically installable updates. Existing pending
/// originals and working values remain the user's comparison basis.
pub fn receive_review(current: &ClientRecord, input: &ReviewInput) -> anyhow::Result<ClientUpdate> {
    current.validate()?;
    input.shared.validate()?;
    anyhow::ensure!(
        input.shared.key() == current.key() && input.original.key() == current.key(),
        "returned review crosses record scope"
    );
    input.local.validate(current.key())?;
    let mut fields = current.clone().into_fields();
    for value in input.retained.iter().chain(match current.latest() {
        Observation::Accepted(value) => Some(value),
        _ => None,
    }) {
        value.validate()?;
        anyhow::ensure!(
            value.key() == current.key(),
            "returned alternative crosses record scope"
        );
        if !fields.retained.contains(value) && input.shared != Observation::Accepted(value.clone())
        {
            fields.retained.push(value.clone());
        }
    }
    fields.latest = input.shared.clone();
    let pending = fields.pending.get_or_insert_with(|| PendingRecord {
        key: current.key(),
        original: input.original.clone(),
        working: current
            .working_state()
            .cloned()
            .unwrap_or_else(|| input.local.clone()),
        prepared: None,
        witness: None,
    });
    let review = ReviewInput {
        original: pending.original.clone(),
        local: pending.working.clone(),
        shared: fields.latest.clone(),
        reason: input.reason.clone(),
        retained: fields.retained.clone(),
    };
    Ok(ClientUpdate {
        record: fields.try_into()?,
        review: Some(review),
    })
}
