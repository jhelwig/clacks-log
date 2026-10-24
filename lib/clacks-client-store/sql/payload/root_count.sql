SELECT (SELECT count(*) FROM qso_payload WHERE payload_id = ?1)
     + (SELECT count(*) FROM tag_payload WHERE payload_id = ?1)
     + (SELECT count(*) FROM profile_payload WHERE payload_id = ?1)
     + (SELECT count(*) FROM equipment_payload WHERE payload_id = ?1)
     + (SELECT count(*) FROM output_profile_payload WHERE payload_id = ?1)
     + (SELECT count(*) FROM group_payload WHERE payload_id = ?1)
     + (SELECT count(*) FROM service_support WHERE payload_id = ?1) AS "roots!: i64"
