SELECT (SELECT count(*) FROM clacks.destination_descriptor d WHERE d.payload_id = $1
            AND NOT EXISTS (SELECT 1 FROM clacks.service_support s WHERE s.payload_id = $1 AND s.descriptor_id = d.descriptor_id)
            AND NOT EXISTS (SELECT 1 FROM clacks.service_destination s WHERE s.payload_id = $1 AND s.descriptor_id = d.descriptor_id))
     + (SELECT count(*) FROM clacks.service_query q WHERE q.payload_id = $1
            AND NOT EXISTS (SELECT 1 FROM clacks.stream_progress s WHERE s.payload_id = $1 AND s.query_id = q.query_id)
            AND NOT EXISTS (SELECT 1 FROM clacks.service_acquisition s WHERE s.payload_id = $1 AND s.query_id = q.query_id)
            AND NOT EXISTS (SELECT 1 FROM clacks.service_observation s WHERE s.payload_id = $1 AND s.query_id = q.query_id))
     + (SELECT count(*) FROM clacks.submission_projection p WHERE p.payload_id = $1
            AND NOT EXISTS (SELECT 1 FROM clacks.service_attempt s WHERE s.payload_id = $1 AND s.projection_id = p.projection_id))
     + (SELECT count(*) FROM clacks.contact_report r WHERE r.payload_id = $1
            AND NOT EXISTS (SELECT 1 FROM clacks.service_observation s WHERE s.payload_id = $1 AND s.contact_report = r.report_id))
     + (SELECT count(*) FROM clacks.station_report r WHERE r.payload_id = $1
            AND NOT EXISTS (SELECT 1 FROM clacks.service_observation s WHERE s.payload_id = $1 AND (s.own_report = r.report_id OR s.counterpart_report = r.report_id))) AS "orphans!: i64"
