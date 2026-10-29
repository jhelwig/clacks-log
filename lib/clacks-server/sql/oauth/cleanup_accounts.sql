SELECT account_id FROM (
    (SELECT account_id FROM clacks.desktop_authorization_codes WHERE expires_at <= $1 ORDER BY expires_at LIMIT 100)
    UNION
    (SELECT account_id FROM clacks.desktop_access_tokens WHERE expires_at <= $1 ORDER BY expires_at LIMIT 100)
    UNION
    (SELECT account_id FROM clacks.desktop_refresh_tokens WHERE expires_at <= $1 ORDER BY expires_at LIMIT 100)
    UNION
    (SELECT account_id FROM clacks.desktop_grants WHERE state='provisional' AND provisional_until <= $1 ORDER BY provisional_until LIMIT 100)
    UNION
    (SELECT account_id FROM clacks.desktop_code_approvals WHERE expires_at <= $1 ORDER BY expires_at LIMIT 100)
) expired ORDER BY account_id LIMIT 100
