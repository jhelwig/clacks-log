UPDATE clacks.desktop_authorization_codes SET receipt_key_version = $3, receipt_nonce = $4, receipt_ciphertext = $5
WHERE account_id = $1 AND code_digest = $2
