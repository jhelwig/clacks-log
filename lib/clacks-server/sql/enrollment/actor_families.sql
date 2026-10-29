SELECT family_id FROM clacks.desktop_grants WHERE account_id=$1 AND actor_id=$2 AND state <> 'revoked' ORDER BY family_id
