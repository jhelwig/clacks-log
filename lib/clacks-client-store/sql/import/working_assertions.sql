SELECT w.record_id
FROM local_working w
JOIN service_assertion a ON a.payload_id = w.payload_id
WHERE w.account = ? AND w.kind = 'imported' AND a.qso_id = ?
ORDER BY w.record_id
