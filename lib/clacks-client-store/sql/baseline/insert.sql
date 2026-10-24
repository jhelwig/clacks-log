INSERT INTO baseline (account, baseline_id, coverage_id, lineage_id, membership_generation) VALUES (?, ?, ?, ?, ?) RETURNING baseline_row AS "baseline_row!: i64"
