SELECT envelope_id AS "envelope_id!: i64" FROM clacks.accepted_head WHERE account = $1 AND kind = $2 AND record_id = $3
