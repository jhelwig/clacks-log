SELECT wrapping_key_reference, count(*) AS "account_keys!"
FROM clacks.account_encryption_keys GROUP BY wrapping_key_reference ORDER BY wrapping_key_reference
