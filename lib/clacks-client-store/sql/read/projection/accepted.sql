SELECT a.envelope_id, a.kind, a.record_id
FROM accepted_record a
JOIN json_each(?2) k ON json_extract(k.value, '$.kind') = a.kind
 AND json_extract(k.value, '$.record_id') = a.record_id
WHERE a.account = ?1
ORDER BY a.envelope_id DESC
