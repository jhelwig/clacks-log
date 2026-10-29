SELECT grant_type AS value FROM clacks.desktop_client_grants WHERE client_id = $1 ORDER BY grant_type
