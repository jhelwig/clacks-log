UPDATE clacks.desktop_clients SET acknowledged_checkpoint_id = $3
WHERE account_id = $1 AND actor_id = $2 AND closed_at IS NULL
