INSERT INTO operating_state (singleton, account, current_values, loaded_profile, profile_record, loaded_equipment, equipment_record)
VALUES (1, ?, ?, ?, ?, ?, ?)
ON CONFLICT (singleton) DO UPDATE SET current_values = excluded.current_values,
    loaded_profile = excluded.loaded_profile, profile_record = excluded.profile_record,
    loaded_equipment = excluded.loaded_equipment, equipment_record = excluded.equipment_record
WHERE account = excluded.account
