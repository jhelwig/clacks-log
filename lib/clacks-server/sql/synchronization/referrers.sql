SELECT record_id AS "record_id!: String", kind AS "kind!: String",
       count(*) OVER () AS "total!: i64"
FROM (
    SELECT h.record_id, h.kind
    FROM clacks.qso_tag t JOIN clacks.accepted_record a ON a.payload_id = t.payload_id AND a.account = t.account
    JOIN clacks.accepted_head h ON h.envelope_id = a.envelope_id AND h.account = a.account
    WHERE h.account = $1 AND h.kind = 'qso' AND t.tag_id = $2 AND $3 = 'tag'
    UNION ALL
    SELECT h.record_id, h.kind
    FROM clacks.profile_payload p JOIN clacks.accepted_record a ON a.payload_id = p.payload_id AND a.account = p.account
    JOIN clacks.accepted_head h ON h.envelope_id = a.envelope_id AND h.account = a.account
    WHERE h.account = $1 AND h.kind = 'profile' AND p.equipment_id = $2 AND $3 = 'equipment'
) AS referrers
ORDER BY kind, record_id LIMIT 10
