SELECT group_id, identifier FROM clacks.directory_reference WHERE payload_id = $1 AND account = $2 AND lookup_key = $3 ORDER BY group_id, identifier
