UPDATE clacks.desktop_grants SET actor_id=$2, state='active', provisional_until=NULL WHERE family_id=$1 AND state='provisional' AND provisional_until > CURRENT_TIMESTAMP
