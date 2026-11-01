SELECT h.kind, h.record_id, a.envelope_id, a.payload_id, a.stamp_id, p.deleted,
       s.timeline_id, s.revision, s.baseline_id, v.body
FROM clacks.accepted_head h
JOIN clacks.accepted_record a USING (envelope_id, account, kind, record_id)
JOIN clacks.payload p USING (payload_id, account, kind, record_id)
JOIN clacks.version_stamp s USING (stamp_id, account, kind, record_id)
JOIN clacks.record_vector v USING (vector_id, account, kind, record_id)
WHERE h.account = $1 ORDER BY h.kind, h.record_id
