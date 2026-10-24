SELECT c.kind,c.record_id FROM client_record c
LEFT JOIN local_review r ON r.account = c.account AND r.kind = c.kind AND r.record_id = c.record_id
LEFT JOIN pending_record p ON p.pending_id = c.pending_id
LEFT JOIN payload w ON w.payload_id = p.working_payload
LEFT JOIN original_basis b ON b.original_id = p.original_id
LEFT JOIN accepted_record original ON original.envelope_id = b.envelope_id
LEFT JOIN record_observation o ON o.observation_id = c.latest_observation
LEFT JOIN accepted_record latest ON latest.envelope_id = o.envelope_id
WHERE c.account = ?1
AND (c.pending_id IS NOT NULL OR r.record_id IS NOT NULL OR EXISTS (SELECT 1 FROM retained_accepted a WHERE a.account = c.account AND a.kind = c.kind AND a.record_id = c.record_id))
AND (?2 IS NULL OR c.kind = ?2)
AND (?3 IS NULL OR EXISTS (SELECT 1 FROM qso_tag t WHERE t.tag_id = ?3 AND t.payload_id = CASE WHEN w.deleted = 0 THEN p.working_payload ELSE coalesce(b.payload_id,original.payload_id,latest.payload_id) END))
AND (?4 IS NULL OR c.kind > ?4 OR (c.kind = ?4 AND c.record_id > ?5))
ORDER BY c.kind,c.record_id LIMIT ?6
