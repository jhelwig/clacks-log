//! Display disambiguation that never changes recorded logbook names.

#[cfg(test)]
mod tests;

use clacks_model::{ids::TagId, tag::LogbookTag};
use std::collections::BTreeMap;

/// Label the complete catalog, extending duplicate-name suffixes until distinct.
pub(in crate::ui) fn labels(tags: &[&LogbookTag]) -> BTreeMap<TagId, String> {
    tags.iter()
        .map(|tag| {
            let peers: Vec<_> = tags
                .iter()
                .filter(|other| other.name == tag.name && other.id != tag.id)
                .map(|other| other.id.to_string())
                .collect();
            if peers.is_empty() {
                return (tag.id, tag.name.clone());
            }
            let id = tag.id.to_string();
            let mut length = 8;
            while length < id.len()
                && peers.iter().any(|peer| {
                    peer.chars()
                        .rev()
                        .take(length)
                        .eq(id.chars().rev().take(length))
                })
            {
                length += 1;
            }
            let suffix: String = id.chars().skip(id.len().saturating_sub(length)).collect();
            let ellipsis = if length < id.len() { "…" } else { "" };
            (tag.id, format!("{} · {ellipsis}{suffix}", tag.name))
        })
        .collect()
}
