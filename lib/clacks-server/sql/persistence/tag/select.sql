SELECT name, lotw_policy, qrz_policy FROM clacks.tag_payload WHERE payload_id = $1 AND account = $2 AND record_id = $3
