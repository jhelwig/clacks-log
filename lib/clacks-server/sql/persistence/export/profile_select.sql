SELECT name, purpose, contextual_fields FROM clacks.output_profile_payload WHERE payload_id = $1 AND account = $2 AND record_id = $3
