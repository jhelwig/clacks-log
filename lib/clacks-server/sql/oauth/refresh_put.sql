INSERT INTO clacks.desktop_refresh_tokens
(refresh_digest, account_id, family_id, client_id, key_thumbprint, grant_established_at, expires_at, state)
VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (refresh_digest) DO UPDATE SET
account_id=EXCLUDED.account_id, family_id=EXCLUDED.family_id, client_id=EXCLUDED.client_id,
key_thumbprint=EXCLUDED.key_thumbprint, grant_established_at=EXCLUDED.grant_established_at,
expires_at=EXCLUDED.expires_at, state=EXCLUDED.state
