DELETE FROM clacks.station_snapshot s WHERE s.account = $1 AND s.station_id IN (
    SELECT s.station_id FROM clacks.station_snapshot s WHERE s.account = $1
      AND NOT EXISTS (SELECT 1 FROM clacks.qso_payload q WHERE q.account = s.account AND (q.own_station = s.station_id OR q.contacted_station = s.station_id))
      AND NOT EXISTS (SELECT 1 FROM clacks.profile_payload p WHERE p.account = s.account AND p.station_id = s.station_id)
    ORDER BY s.station_id LIMIT $2
)
