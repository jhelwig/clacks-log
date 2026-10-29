SELECT account_id, actor_id, client_id, key_thumbprint, grant_established_at, state, provisional_until FROM clacks.desktop_grants WHERE family_id = $1 FOR UPDATE
