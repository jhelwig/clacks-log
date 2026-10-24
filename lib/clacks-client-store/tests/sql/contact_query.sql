SELECT q.payload_id AS "payload_id!: i64", q.frequency, q.power,
       (SELECT count(*) FROM qso_tag t WHERE t.payload_id = q.payload_id) AS "tags!: i64",
       (SELECT count(*) FROM station_reference r WHERE r.station_id = q.own_station) AS "references!: i64"
FROM qso_payload q WHERE q.frequency >= ? AND q.power <= ? ORDER BY q.payload_id
