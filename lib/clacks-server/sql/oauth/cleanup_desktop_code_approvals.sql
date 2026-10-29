DELETE FROM clacks.desktop_code_approvals WHERE code_digest IN (SELECT code_digest FROM clacks.desktop_code_approvals WHERE account_id=$1 AND expires_at <= $2 LIMIT 500)
