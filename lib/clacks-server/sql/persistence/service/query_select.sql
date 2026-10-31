SELECT destination_id, stream, own_call, since, selected_fields FROM clacks.service_query WHERE query_id = $1 AND payload_id = $2 AND account = $3
