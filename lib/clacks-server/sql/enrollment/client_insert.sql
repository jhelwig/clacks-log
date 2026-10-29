INSERT INTO clacks.desktop_clients
(account_id,actor_id,label,authorized_at,created_timeline,created_revision)
VALUES ($1,$2,$3,CURRENT_TIMESTAMP,$4,$5)
