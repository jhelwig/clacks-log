SELECT kind AS "kind!: String", record_id AS "record_id!: String" FROM (
    SELECT kind, record_id FROM clacks.accepted_head WHERE account = $1
    UNION SELECT kind, record_id FROM clacks.retained_accepted WHERE account = $1
) AS identities
WHERE (kind, record_id) > ($2,$3)
ORDER BY kind, record_id LIMIT $4
