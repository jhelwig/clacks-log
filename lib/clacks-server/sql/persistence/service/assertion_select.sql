SELECT qso_id, service, destination_id, claim, date, source, source_record, mapping, native_end, fields, has_location, coverage FROM clacks.service_assertion WHERE payload_id = $1 AND account = $2
