DELETE FROM clacks.desktop_access_tokens WHERE access_digest IN (SELECT access_digest FROM clacks.desktop_access_tokens WHERE account_id=$1 AND expires_at <= $2 LIMIT 500)
