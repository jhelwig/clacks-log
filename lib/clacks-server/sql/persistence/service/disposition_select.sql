SELECT observation_id, decision, association_id FROM clacks.service_disposition WHERE payload_id = $1 AND account = $2
