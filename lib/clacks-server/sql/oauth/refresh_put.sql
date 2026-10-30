INSERT INTO clacks.desktop_refresh_tokens
(refresh_digest, account_id, family_id, expires_at, state)
VALUES ($1,$2,$3,$4,$5) ON CONFLICT (refresh_digest) DO UPDATE SET
account_id=EXCLUDED.account_id, family_id=EXCLUDED.family_id,
expires_at=EXCLUDED.expires_at, state=EXCLUDED.state
