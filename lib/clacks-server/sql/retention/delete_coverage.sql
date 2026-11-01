DELETE FROM clacks.coverage c WHERE c.account = $1 AND c.coverage_id IN (
    SELECT c.coverage_id FROM clacks.coverage c WHERE c.account = $1
      AND NOT EXISTS (SELECT 1 FROM clacks.sync_accounts a WHERE a.account_id = c.account AND (a.installed_coverage_id = c.coverage_id OR a.retained_journal_coverage_id = c.coverage_id))
      AND NOT EXISTS (SELECT 1 FROM clacks.sync_baselines b WHERE b.account_id = c.account AND b.coverage_id = c.coverage_id)
      AND NOT EXISTS (SELECT 1 FROM clacks.checkpoint p WHERE p.account = c.account AND p.coverage_id = c.coverage_id)
      AND NOT EXISTS (SELECT 1 FROM clacks.timeline_boundary t WHERE t.account = c.account AND t.coverage_id = c.coverage_id)
    ORDER BY c.coverage_id LIMIT $2
)
