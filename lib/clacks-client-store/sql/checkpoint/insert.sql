INSERT INTO checkpoint (account, baseline_id, coverage_id) VALUES (?, ?, ?) RETURNING checkpoint_id AS "checkpoint_id!: i64"
