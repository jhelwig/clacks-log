UPDATE clacks.desktop_authorization_codes SET receipt = $3
WHERE account_id = $1 AND code_digest = $2
