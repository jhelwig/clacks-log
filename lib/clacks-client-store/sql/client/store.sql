INSERT INTO client_record (account, kind, record_id, latest_observation, pending_id) VALUES (?, ?, ?, ?, ?)
ON CONFLICT (account, kind, record_id) DO UPDATE SET latest_observation = excluded.latest_observation, pending_id = excluded.pending_id
