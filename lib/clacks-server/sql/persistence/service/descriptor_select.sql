SELECT destination_id, collection, lotw_account, qrz_book, description FROM clacks.destination_descriptor WHERE descriptor_id = $1 AND payload_id = $2 AND account = $3
