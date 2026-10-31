SELECT account_id, actor_id, label, authorized_at, closed_at, created_timeline, created_revision, closed_timeline, closed_revision, acknowledged_checkpoint_id
FROM clacks.desktop_clients WHERE account_id = $1 ORDER BY authorized_at, actor_id
