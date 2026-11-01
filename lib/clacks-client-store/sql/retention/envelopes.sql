SELECT a.envelope_id, a.kind, a.record_id, p.deleted, s.timeline_id, s.revision, s.baseline_id, v.body,
       (EXISTS (SELECT 1 FROM record_observation o WHERE o.account = a.account AND o.envelope_id = a.envelope_id)
        OR EXISTS (SELECT 1 FROM original_basis o WHERE o.account = a.account AND o.envelope_id = a.envelope_id)
        OR EXISTS (SELECT 1 FROM retained_accepted r WHERE r.account = a.account AND r.envelope_id = a.envelope_id)
        OR EXISTS (SELECT 1 FROM staged_accepted r WHERE r.account = a.account AND r.envelope_id = a.envelope_id)
        OR EXISTS (SELECT 1 FROM review_retained r WHERE r.account = a.account AND r.envelope_id = a.envelope_id)
        OR EXISTS (SELECT 1 FROM current_external_binding b WHERE b.account = a.account AND b.envelope_id = a.envelope_id)) AS "rooted!: i64"
FROM accepted_record a
JOIN payload p USING (payload_id, account, kind, record_id)
JOIN version_stamp s USING (stamp_id, account, kind, record_id)
JOIN record_vector v USING (vector_id, account, kind, record_id)
WHERE a.account = ?1 ORDER BY a.envelope_id DESC
