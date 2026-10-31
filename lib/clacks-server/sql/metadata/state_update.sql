UPDATE clacks.sync_accounts SET timeline_id = $2, revision = $3, baseline_id = $4, membership_generation = $5, installed_coverage_id = $6 WHERE account_id = $1
