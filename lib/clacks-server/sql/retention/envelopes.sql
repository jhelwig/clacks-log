SELECT a.envelope_id, a.kind, a.record_id, p.deleted, s.timeline_id, s.revision, s.baseline_id, v.body,
       (EXISTS (SELECT 1 FROM clacks.accepted_head h WHERE h.account = a.account AND h.envelope_id = a.envelope_id)
        OR EXISTS (SELECT 1 FROM clacks.retained_accepted r WHERE r.account = a.account AND r.envelope_id = a.envelope_id)
        OR EXISTS (SELECT 1 FROM clacks.staged_accepted r WHERE r.account = a.account AND r.envelope_id = a.envelope_id)
        OR EXISTS (SELECT 1 FROM clacks.current_external_binding b WHERE b.account = a.account AND b.envelope_id = a.envelope_id)) AS "rooted!: bool"
FROM clacks.accepted_record a
JOIN clacks.payload p USING (payload_id, account, kind, record_id)
JOIN clacks.version_stamp s USING (stamp_id, account, kind, record_id)
JOIN clacks.record_vector v USING (vector_id, account, kind, record_id)
WHERE a.account = $1 ORDER BY a.envelope_id DESC
