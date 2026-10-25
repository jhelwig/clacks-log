mod support;

use clacks_model::ids::{QsoId, TagId};
use clacks_model::tag::{
    AutomaticUploadPolicy, LogbookTag, TagDeletionChoice, automatic_upload_allowed,
    plan_tag_deletion,
};

#[test]
fn upload_policy_truth_table() {
    use AutomaticUploadPolicy::*;
    assert!(!automatic_upload_allowed([]));
    for a in [Neither, Enable, Forbid] {
        for b in [Neither, Enable, Forbid] {
            let expected = a != Forbid && b != Forbid && (a == Enable || b == Enable);
            assert_eq!(automatic_upload_allowed([a, b]), expected);
        }
    }
    let t = LogbookTag::new(support::id::<TagId>(1), "test".into());
    assert_eq!(t.automatic_upload.lotw, Neither);
    assert_eq!(t.automatic_upload.qrz, Neither);
}

#[test]
fn tag_deletion_counts_identities() {
    let tag = support::id::<TagId>(1);
    let mut qsos: Vec<_> = (1..=100)
        .map(|n| {
            let mut q = support::qso(support::id::<QsoId>(n));
            q.tags.insert(tag);
            if n <= 30 {
                q.tags.insert(support::id::<TagId>(2));
            }
            q
        })
        .collect();
    qsos.push(qsos[0].clone());
    for (choice, deleted, untagged) in [
        (TagDeletionChoice::LeaveAll, 0, 100),
        (TagDeletionChoice::DeleteAll, 100, 0),
        (TagDeletionChoice::DeleteUnique, 70, 30),
    ] {
        let plan = plan_tag_deletion(tag, choice, &qsos);
        assert_eq!(plan.affected.len(), 100);
        assert_eq!(plan.also_tagged_elsewhere, 30);
        assert_eq!(plan.mark_deleted.len(), deleted);
        assert_eq!(plan.remove_membership.len(), untagged);
        assert!(plan.mark_deleted.is_disjoint(&plan.remove_membership));
    }
    assert!(qsos.iter().all(|q| q.tags.contains(&tag)));
}
