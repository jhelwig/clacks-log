INSERT INTO clacks.desktop_authorization_codes
(code_digest, account_id, client_id, redirect_uri, redirect_explicit, code_challenge,
 key_thumbprint, issued_at, expires_at, state, receipt_key_version, receipt_nonce, receipt_ciphertext)
VALUES ($1, $2, 'clacks-desktop', 'http://127.0.0.1:1234/oauth/callback', TRUE, repeat('A', 43),
 'test-key', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP + INTERVAL '60 seconds', 'consumed', $3, $4, $5)
