SELECT group_id, identifier FROM clacks.station_reference WHERE station_id = $1 AND account = $2 ORDER BY group_id, identifier
