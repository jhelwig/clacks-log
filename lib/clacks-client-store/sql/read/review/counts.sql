WITH records AS (
SELECT (r.record_id IS NOT NULL OR c.authorization_hold IS NOT NULL OR EXISTS (SELECT 1 FROM retained_accepted a WHERE a.account=c.account AND a.kind=c.kind AND a.record_id=c.record_id)) AS blocked
FROM client_record c
LEFT JOIN local_review r ON r.account=c.account AND r.kind=c.kind AND r.record_id=c.record_id
WHERE c.account=?1 AND (?2 IS NULL OR c.kind=?2)
AND (c.pending_id IS NOT NULL OR r.record_id IS NOT NULL OR c.authorization_hold IS NOT NULL OR EXISTS (SELECT 1 FROM retained_accepted a WHERE a.account=c.account AND a.kind=c.kind AND a.record_id=c.record_id))
)
SELECT count(CASE WHEN blocked THEN 1 END) AS conflicts,
count(CASE WHEN NOT blocked THEN 1 END) AS pending_sync FROM records
