INSERT INTO operating_value (account, station_id, operator_call, operator_name, power, equipment_id) VALUES (?, ?, ?, ?, ?, ?) RETURNING operating_id AS "operating_id!: i64"
