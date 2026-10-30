SELECT (SELECT count(*) FROM destination_descriptor d WHERE d.payload_id = ?1
            AND NOT EXISTS (SELECT 1 FROM service_support s WHERE s.payload_id = ?1 AND s.descriptor_id = d.descriptor_id)
            AND NOT EXISTS (SELECT 1 FROM service_destination s WHERE s.payload_id = ?1 AND s.descriptor_id = d.descriptor_id))
     + (SELECT count(*) FROM service_query q WHERE q.payload_id = ?1
            AND NOT EXISTS (SELECT 1 FROM stream_progress s WHERE s.payload_id = ?1 AND s.query_id = q.query_id)
            AND NOT EXISTS (SELECT 1 FROM service_acquisition s WHERE s.payload_id = ?1 AND s.query_id = q.query_id)
            AND NOT EXISTS (SELECT 1 FROM service_observation s WHERE s.payload_id = ?1 AND s.query_id = q.query_id))
     + (SELECT count(*) FROM submission_projection p WHERE p.payload_id = ?1
            AND NOT EXISTS (SELECT 1 FROM service_attempt s WHERE s.payload_id = ?1 AND s.projection_id = p.projection_id))
     + (SELECT count(*) FROM contact_report r WHERE r.payload_id = ?1
            AND NOT EXISTS (SELECT 1 FROM service_observation s WHERE s.payload_id = ?1 AND s.contact_report = r.report_id))
     + (SELECT count(*) FROM station_report r WHERE r.payload_id = ?1
            AND NOT EXISTS (SELECT 1 FROM service_observation s WHERE s.payload_id = ?1 AND (s.own_report = r.report_id OR s.counterpart_report = r.report_id))) AS "orphans!: i64"
