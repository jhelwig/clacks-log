SELECT envelope_id FROM clacks.retained_accepted WHERE account = $1 AND kind = $2 AND record_id = $3 ORDER BY envelope_id
