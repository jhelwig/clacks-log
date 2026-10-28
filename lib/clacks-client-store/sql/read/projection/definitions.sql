SELECT w.kind, w.record_id, w.payload_id AS "payload_id!"
FROM local_working w JOIN payload p ON p.payload_id = w.payload_id
WHERE w.account = ?1 AND w.kind IN ('group', 'output_profile') AND p.deleted = 0
ORDER BY w.kind, w.record_id
