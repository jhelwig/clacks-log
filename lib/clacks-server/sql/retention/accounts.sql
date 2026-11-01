SELECT account_id FROM clacks.sync_accounts WHERE account_id > $1 ORDER BY account_id LIMIT $2
