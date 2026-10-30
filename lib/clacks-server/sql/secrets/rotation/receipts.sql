SELECT c.account_id, c.code_digest, a.purpose AS "purpose?", c.receipt_key_version AS key_version,
       c.receipt_nonce AS nonce, c.receipt_ciphertext AS ciphertext
FROM clacks.desktop_authorization_codes c
LEFT JOIN clacks.desktop_code_approvals a ON a.code_digest = c.code_digest AND a.account_id = c.account_id
WHERE c.account_id = $1 AND c.receipt_key_version = $2
ORDER BY c.code_digest LIMIT $3
