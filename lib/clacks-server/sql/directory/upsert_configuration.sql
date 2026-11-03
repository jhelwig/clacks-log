INSERT INTO clacks.directory_configuration(account_id, revision, selected_provider) VALUES ($1, $2, $3)
ON CONFLICT(account_id) DO UPDATE SET revision = EXCLUDED.revision, selected_provider = EXCLUDED.selected_provider
