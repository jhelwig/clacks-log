INSERT INTO clacks.desktop_grants
(account_id, family_id, client_id, grant_established_at, key_thumbprint, state, provisional_until)
VALUES ($1,$2,$3,$4,$5,'provisional', CURRENT_TIMESTAMP + INTERVAL '5 minutes')
ON CONFLICT (family_id) DO NOTHING
