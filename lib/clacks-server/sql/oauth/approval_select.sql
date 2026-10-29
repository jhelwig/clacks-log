SELECT account_id, purpose, target_actor, label, expires_at FROM clacks.desktop_code_approvals WHERE code_digest = $1
