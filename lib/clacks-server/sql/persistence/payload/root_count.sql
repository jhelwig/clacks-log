SELECT (SELECT count(*) FROM clacks.qso_payload WHERE payload_id = $1)
     + (SELECT count(*) FROM clacks.tag_payload WHERE payload_id = $1)
     + (SELECT count(*) FROM clacks.profile_payload WHERE payload_id = $1)
     + (SELECT count(*) FROM clacks.equipment_payload WHERE payload_id = $1)
     + (SELECT count(*) FROM clacks.output_profile_payload WHERE payload_id = $1)
     + (SELECT count(*) FROM clacks.group_payload WHERE payload_id = $1)
     + (SELECT count(*) FROM clacks.service_support WHERE payload_id = $1) AS "roots!: i64"
