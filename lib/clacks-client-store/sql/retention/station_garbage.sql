DELETE FROM station_snapshot
WHERE NOT EXISTS (SELECT 1 FROM qso_payload q WHERE q.own_station = station_snapshot.station_id OR q.contacted_station = station_snapshot.station_id)
AND NOT EXISTS (SELECT 1 FROM profile_payload p WHERE p.station_id = station_snapshot.station_id)
AND NOT EXISTS (SELECT 1 FROM operating_value o WHERE o.station_id = station_snapshot.station_id)
