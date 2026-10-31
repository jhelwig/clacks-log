INSERT INTO clacks.sync_baselines (account_id, baseline_id, coverage_id, lineage_id, membership_generation, root_timeline)
VALUES ($1, $2, $3, $4, $5, (SELECT initial_timeline FROM clacks.lineage WHERE lineage_id = $4 AND account = $1))
RETURNING baseline_row AS "baseline_row!: i64"
