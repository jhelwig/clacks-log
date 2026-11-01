DELETE FROM clacks.checkpoint c WHERE c.account = $1 AND c.checkpoint_id IN (
    SELECT c.checkpoint_id FROM clacks.checkpoint c WHERE c.account = $1
      AND NOT EXISTS (SELECT 1 FROM clacks.desktop_clients d WHERE d.account_id = c.account AND d.acknowledged_checkpoint_id = c.checkpoint_id)
    ORDER BY c.checkpoint_id LIMIT $2
)
