INSERT INTO local_state (key, value) VALUES ('logbooks.selected_view', '"unsupported"')
ON CONFLICT (key) DO UPDATE SET value = excluded.value;
