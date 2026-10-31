INSERT INTO clacks.checkpoint (account, baseline_id, coverage_id) VALUES ($1, $2, $3) RETURNING checkpoint_id AS "checkpoint_id!: i64"
