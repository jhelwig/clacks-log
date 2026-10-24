INSERT INTO equipment_snapshot (account, equipment_description, antenna_description, notes) VALUES (?, ?, ?, ?) RETURNING equipment_id AS "equipment_id!: i64"
