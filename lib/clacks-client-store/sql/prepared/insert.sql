INSERT INTO prepared_change (account, kind, record_id, payload_id, vector_id, actor_id, condition_id) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING prepared_id AS "prepared_id!: i64"
