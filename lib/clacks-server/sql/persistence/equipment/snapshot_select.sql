SELECT equipment_description, antenna_description, notes FROM clacks.equipment_snapshot WHERE equipment_id = $1 AND account = $2
