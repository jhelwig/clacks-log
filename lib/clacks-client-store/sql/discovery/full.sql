WITH facts AS (
    SELECT c.account,c.kind,c.record_id,o.envelope_id FROM client_record c
    JOIN record_observation o ON o.observation_id=c.latest_observation
    WHERE c.account=?1 AND o.envelope_id IS NOT NULL
    UNION SELECT account,kind,record_id,envelope_id FROM retained_accepted WHERE account=?1
), identities AS (
    SELECT f.kind,f.record_id FROM facts f
    JOIN accepted_record a ON a.envelope_id=f.envelope_id AND a.account=f.account
    JOIN version_stamp s ON s.stamp_id=a.stamp_id
    WHERE NOT EXISTS (SELECT 1 FROM json_each(?6) known
        WHERE json_extract(known.value,'$[0]')=s.timeline_id
          AND json_extract(known.value,'$[1]')>=s.revision)
    UNION SELECT kind,record_id FROM staged_accepted WHERE account=?1 AND ?2
)
SELECT kind AS "kind!: String",record_id AS "record_id!: String" FROM identities
WHERE (kind,record_id)>(?3,?4)
ORDER BY kind,record_id LIMIT ?5
