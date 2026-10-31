SELECT c.kind,c.record_id
FROM client_record c
LEFT JOIN local_review r ON r.account=c.account AND r.kind=c.kind AND r.record_id=c.record_id
WHERE c.account=?1
AND (c.pending_id IS NOT NULL OR r.record_id IS NOT NULL OR c.authorization_hold IS NOT NULL OR EXISTS (SELECT 1 FROM retained_accepted a WHERE a.account=c.account AND a.kind=c.kind AND a.record_id=c.record_id))
AND (?2 IS NULL OR c.kind=?2)
AND (?3 IS NULL OR ?3=(r.record_id IS NOT NULL OR c.authorization_hold IS NOT NULL OR EXISTS (SELECT 1 FROM retained_accepted a WHERE a.account=c.account AND a.kind=c.kind AND a.record_id=c.record_id)))
AND (?4 IS NULL OR c.kind>?4 OR (c.kind=?4 AND c.record_id>?5))
ORDER BY c.kind,c.record_id LIMIT ?6
