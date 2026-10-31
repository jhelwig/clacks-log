INSERT INTO clacks.station_snapshot (account, callsign) VALUES ($1, $2) RETURNING station_id AS "station_id!: i64"
