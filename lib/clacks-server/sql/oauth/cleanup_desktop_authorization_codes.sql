DELETE FROM clacks.desktop_authorization_codes WHERE code_digest IN (SELECT code_digest FROM clacks.desktop_authorization_codes WHERE account_id=$1 AND expires_at <= $2 LIMIT 500)
