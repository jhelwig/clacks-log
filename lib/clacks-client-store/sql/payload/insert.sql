INSERT INTO payload (account, kind, record_id, deleted) VALUES (?, ?, ?, ?) RETURNING payload_id AS "payload_id!: i64"
