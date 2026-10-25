UPDATE operating_state SET
    loaded_profile = CASE WHEN ?2 = 'profile' AND profile_record = ?3 THEN NULL ELSE loaded_profile END,
    profile_record = CASE WHEN ?2 = 'profile' AND profile_record = ?3 THEN NULL ELSE profile_record END,
    loaded_equipment = CASE WHEN ?2 = 'equipment' AND equipment_record = ?3 THEN NULL ELSE loaded_equipment END,
    equipment_record = CASE WHEN ?2 = 'equipment' AND equipment_record = ?3 THEN NULL ELSE equipment_record END
WHERE account = ?1
