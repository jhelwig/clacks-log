INSERT INTO pending_record (account, kind, record_id, original_id, working_payload, prepared_id, witness_id) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING pending_id AS "pending_id!: i64"
