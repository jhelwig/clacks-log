SELECT external_id AS "external_id!: i64" FROM clacks.external_key WHERE account = $1 AND destination_id = $2 AND canonical_key = $3
