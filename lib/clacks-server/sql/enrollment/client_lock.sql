SELECT closed_at AS "closed_at: jiff_sqlx::Timestamp" FROM clacks.desktop_clients WHERE account_id=$1 AND actor_id=$2 FOR UPDATE
