INSERT INTO record_observation (account, kind, record_id, variant, envelope_id, checkpoint_id, unknown_reason) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING observation_id AS "observation_id!: i64"
