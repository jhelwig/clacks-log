UPDATE clacks.desktop_clients SET closed_at=CURRENT_TIMESTAMP, closed_timeline=$3, closed_revision=$4 WHERE account_id=$1 AND actor_id=$2 AND closed_at IS NULL
