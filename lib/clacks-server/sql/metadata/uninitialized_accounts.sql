SELECT a.account_id
FROM clacks.accounts a
WHERE NOT EXISTS (SELECT 1 FROM clacks.sync_accounts s WHERE s.account_id = a.account_id)
ORDER BY a.account_id
