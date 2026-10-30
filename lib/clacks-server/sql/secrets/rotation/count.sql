SELECT (SELECT count(*) FROM clacks.service_credentials WHERE account_id = $1 AND key_version = $2) AS "credentials!",
       (SELECT count(*) FROM clacks.desktop_authorization_codes WHERE account_id = $1 AND receipt_key_version = $2) AS "receipts!"
