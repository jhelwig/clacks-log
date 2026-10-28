FROM local_qso q
JOIN qso_payload p ON p.payload_id = q.payload_id
LEFT JOIN recorded_location ol ON ol.station_id = p.own_station
LEFT JOIN recorded_location cl ON cl.station_id = p.contacted_station
LEFT JOIN equipment_snapshot e ON e.equipment_id = p.equipment_id
LEFT JOIN local_review review ON review.account = q.account AND review.kind = 'qso' AND review.record_id = q.record_id
LEFT JOIN contact_query_facts f ON f.qso_id = q.record_id
