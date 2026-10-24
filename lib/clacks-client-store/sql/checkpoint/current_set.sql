INSERT INTO installed_checkpoint (singleton, account, checkpoint_id) VALUES (1, ?, ?) ON CONFLICT (singleton) DO UPDATE SET checkpoint_id = excluded.checkpoint_id WHERE account = excluded.account
