INSERT INTO local_state (key, value) VALUES (?, ?)
ON CONFLICT (key) DO UPDATE SET value = excluded.value
