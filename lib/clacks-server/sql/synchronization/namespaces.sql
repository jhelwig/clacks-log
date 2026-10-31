SELECT DISTINCT destination_id, collection, lotw_account, qrz_book, description
FROM clacks.destination_descriptor WHERE account = $1 ORDER BY destination_id
