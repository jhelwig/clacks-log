INSERT INTO clacks.desktop_authorization_codes
(code_digest, account_id, client_id, redirect_uri, redirect_explicit, code_challenge,
key_thumbprint, issued_at, expires_at, state, receipt)
VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
ON CONFLICT (code_digest) DO UPDATE SET
account_id=EXCLUDED.account_id, client_id=EXCLUDED.client_id, redirect_uri=EXCLUDED.redirect_uri,
redirect_explicit=EXCLUDED.redirect_explicit, code_challenge=EXCLUDED.code_challenge,
key_thumbprint=EXCLUDED.key_thumbprint, issued_at=EXCLUDED.issued_at, expires_at=EXCLUDED.expires_at,
state=EXCLUDED.state, receipt=EXCLUDED.receipt
