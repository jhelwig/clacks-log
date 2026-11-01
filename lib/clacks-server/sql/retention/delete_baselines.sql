DELETE FROM clacks.sync_baselines b WHERE b.account_id = $1 AND b.baseline_row IN (
    SELECT b.baseline_row FROM clacks.sync_baselines b WHERE b.account_id = $1
      AND NOT EXISTS (SELECT 1 FROM clacks.sync_accounts a WHERE a.account_id = b.account_id AND a.baseline_id = b.baseline_id)
      AND NOT EXISTS (SELECT 1 FROM clacks.version_stamp s WHERE s.account = b.account_id AND s.baseline_id = b.baseline_id)
      AND NOT EXISTS (SELECT 1 FROM clacks.checkpoint c WHERE c.account = b.account_id AND c.baseline_id = b.baseline_id)
    ORDER BY b.baseline_row LIMIT $2
)
