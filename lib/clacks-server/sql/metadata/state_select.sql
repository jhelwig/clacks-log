SELECT timeline_id, revision, baseline_id, membership_generation, installed_coverage_id, current_lineage_id, retained_journal_coverage_id
FROM clacks.sync_accounts WHERE account_id = $1
