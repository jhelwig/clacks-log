INSERT INTO accepted_record (account, kind, record_id, payload_id, stamp_id) VALUES (?, ?, ?, ?, ?) RETURNING envelope_id AS "envelope_id!: i64"
