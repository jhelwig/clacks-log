INSERT INTO clacks.desktop_oauth_clients (client_id, display_name) VALUES ($1, $2) ON CONFLICT (client_id) DO UPDATE SET display_name = EXCLUDED.display_name
