SELECT k.key_version AS version, k.active, k.wrapping_key_reference,
       (SELECT count(*) FROM clacks.desktop_authorization_codes c WHERE c.account_id = k.account_id AND c.receipt_key_version = k.key_version) AS "receipts!",
       (SELECT count(*) FROM clacks.directory_material d WHERE d.account_id = k.account_id AND d.account_key_version = k.key_version) AS "directory_values!"
FROM clacks.account_encryption_keys k WHERE k.account_id = $1 ORDER BY k.key_version
