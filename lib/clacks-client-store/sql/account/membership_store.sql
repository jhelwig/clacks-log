INSERT INTO membership_snapshot (account, generation) VALUES (?, ?) ON CONFLICT (account) DO UPDATE SET generation = excluded.generation
