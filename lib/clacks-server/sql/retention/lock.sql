SELECT account_id FROM clacks.accounts WHERE account_id = $1 FOR UPDATE SKIP LOCKED
