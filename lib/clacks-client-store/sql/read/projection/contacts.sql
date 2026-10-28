SELECT q.record_id, q.payload_id, q.pending AS "pending!: bool"
FROM local_qso q
WHERE q.account = ?1 AND q.record_id IN (SELECT value FROM json_each(?2))
ORDER BY q.record_id
