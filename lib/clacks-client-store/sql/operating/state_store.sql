INSERT INTO operating_state (singleton, account, current_values, loaded_profile, profile_record, loaded_values) VALUES (1, ?, ?, ?, ?, ?)
ON CONFLICT (singleton) DO UPDATE SET current_values = excluded.current_values, loaded_profile = excluded.loaded_profile, profile_record = excluded.profile_record, loaded_values = excluded.loaded_values WHERE account = excluded.account
