SELECT wrapping_key_id, count(*) AS "account_keys!"
FROM clacks.account_encryption_keys GROUP BY wrapping_key_id ORDER BY wrapping_key_id
