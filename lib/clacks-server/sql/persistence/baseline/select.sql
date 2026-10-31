SELECT baseline_id, coverage_id, lineage_id, membership_generation FROM clacks.sync_baselines WHERE baseline_row = $1 AND account_id = $2
