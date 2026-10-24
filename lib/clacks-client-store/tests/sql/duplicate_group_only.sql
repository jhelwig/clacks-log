INSERT INTO station_reference (station_id, account, group_id, identifier)
SELECT own_station, account, ?, NULL FROM qso_payload WHERE payload_id = ?
