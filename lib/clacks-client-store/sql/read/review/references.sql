WITH referrers AS (
    SELECT c.kind,c.record_id
    FROM client_record c
    JOIN record_observation o ON o.observation_id=c.latest_observation
    JOIN accepted_record a ON a.envelope_id=o.envelope_id
    JOIN payload p ON p.payload_id=a.payload_id
    JOIN qso_tag t ON t.payload_id=p.payload_id
    WHERE c.account=?1 AND ?2='tag' AND t.tag_id=?3 AND p.deleted=0
    UNION
    SELECT c.kind,c.record_id
    FROM client_record c
    JOIN record_observation o ON o.observation_id=c.latest_observation
    JOIN accepted_record a ON a.envelope_id=o.envelope_id
    JOIN payload p ON p.payload_id=a.payload_id
    JOIN profile_payload t ON t.payload_id=p.payload_id
    WHERE c.account=?1 AND ?2='equipment' AND t.equipment_id=?3 AND p.deleted=0
)
SELECT kind,record_id FROM referrers
WHERE (?4 IS NULL OR kind>?4 OR (kind=?4 AND record_id>?5))
ORDER BY kind,record_id LIMIT ?6
