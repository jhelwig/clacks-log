SELECT s.stamp_id, s.kind, s.record_id, s.timeline_id, s.revision, s.baseline_id, v.body
FROM clacks.version_stamp s JOIN clacks.record_vector v USING (vector_id, account, kind, record_id)
WHERE s.account = $1 ORDER BY s.stamp_id DESC
