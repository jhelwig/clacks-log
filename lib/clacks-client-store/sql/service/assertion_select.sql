SELECT qso_id, service, destination_id, claim, date, source, source_record, mapping, native_end, fields, has_location, coverage FROM service_assertion WHERE payload_id = ? AND account = ?
