SELECT k.key_version AS version, k.active, k.wrapping_key_id,
       (SELECT count(*) FROM clacks.service_credentials c WHERE c.account_id = k.account_id AND c.key_version = k.key_version) AS "credentials!",
       (SELECT count(*) FROM clacks.desktop_authorization_codes c WHERE c.account_id = k.account_id AND c.receipt_key_version = k.key_version) AS "receipts!"
FROM clacks.account_encryption_keys k WHERE k.account_id = $1 ORDER BY k.key_version
