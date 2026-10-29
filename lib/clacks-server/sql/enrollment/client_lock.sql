SELECT closed_at FROM clacks.desktop_clients WHERE account_id=$1 AND actor_id=$2 FOR UPDATE
