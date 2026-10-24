INSERT INTO current_lineage (account, lineage_id) VALUES (?, ?) ON CONFLICT (account) DO UPDATE SET lineage_id = excluded.lineage_id
