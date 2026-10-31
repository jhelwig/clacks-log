INSERT INTO clacks.payload (account, kind, record_id, deleted) VALUES ($1, $2, $3, $4) RETURNING payload_id AS "payload_id!: i64"
