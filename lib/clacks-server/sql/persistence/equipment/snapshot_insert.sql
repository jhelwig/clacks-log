INSERT INTO clacks.equipment_snapshot (account, equipment_description, antenna_description, notes) VALUES ($1, $2, $3, $4) RETURNING equipment_id AS "equipment_id!: i64"
