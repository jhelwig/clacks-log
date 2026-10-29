DELETE FROM clacks.desktop_refresh_tokens WHERE refresh_digest IN (SELECT refresh_digest FROM clacks.desktop_refresh_tokens WHERE account_id=$1 AND expires_at <= $2 LIMIT 500)
