INSERT INTO clacks.accepted_record (account, kind, record_id, payload_id, stamp_id) VALUES ($1, $2, $3, $4, $5) RETURNING envelope_id AS "envelope_id!: i64"
