INSERT INTO local_state (key, value)
VALUES ('logbooks.selected_view', '{"Logbook":"00000000000000000000000033"}')
ON CONFLICT (key) DO UPDATE SET value = excluded.value;
