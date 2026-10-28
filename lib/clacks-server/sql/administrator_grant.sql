INSERT INTO clacks.server_administrators (account_id) VALUES ($1) ON CONFLICT DO NOTHING
