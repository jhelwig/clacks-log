DELETE FROM clacks.lineage l WHERE l.account = $1 AND l.lineage_id IN (
    SELECT l.lineage_id FROM clacks.lineage l WHERE l.account = $1
      AND NOT EXISTS (SELECT 1 FROM clacks.sync_accounts a WHERE a.account_id = l.account AND a.current_lineage_id = l.lineage_id)
      AND NOT EXISTS (SELECT 1 FROM clacks.sync_baselines b WHERE b.account_id = l.account AND b.lineage_id = l.lineage_id)
    ORDER BY l.lineage_id LIMIT $2
)
