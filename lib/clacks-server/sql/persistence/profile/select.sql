SELECT name, station_id, equipment_id FROM clacks.profile_payload WHERE payload_id = $1 AND account = $2 AND record_id = $3
