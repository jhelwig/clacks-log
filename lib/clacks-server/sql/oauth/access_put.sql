INSERT INTO clacks.desktop_access_tokens
(access_digest, account_id, family_id, client_id, key_thumbprint, issued_at, grant_established_at, expires_at)
VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (access_digest) DO UPDATE SET
account_id=EXCLUDED.account_id, family_id=EXCLUDED.family_id, client_id=EXCLUDED.client_id,
key_thumbprint=EXCLUDED.key_thumbprint, issued_at=EXCLUDED.issued_at,
grant_established_at=EXCLUDED.grant_established_at, expires_at=EXCLUDED.expires_at
