SELECT operation_id, acquisition_stamp, descriptor_id FROM clacks.service_support WHERE payload_id = $1 AND account = $2 AND kind = $3 AND record_id = $4
