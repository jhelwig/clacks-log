INSERT INTO local_counter (account, kind, record_id, actor_id, high_water) VALUES (?, ?, ?, ?, ?)
ON CONFLICT (account, kind, record_id, actor_id) DO UPDATE SET high_water = excluded.high_water WHERE excluded.high_water >= high_water
