UPDATE clacks.current_external_binding SET envelope_id = $3
WHERE account = $1 AND envelope_id = $2
