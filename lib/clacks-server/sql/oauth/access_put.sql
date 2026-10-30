INSERT INTO clacks.desktop_access_tokens
(access_digest, account_id, family_id, issued_at, expires_at)
VALUES ($1,$2,$3,$4,$5) ON CONFLICT (access_digest) DO UPDATE SET
account_id=EXCLUDED.account_id, family_id=EXCLUDED.family_id,
issued_at=EXCLUDED.issued_at, expires_at=EXCLUDED.expires_at
