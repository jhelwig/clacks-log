UPDATE clacks.installation_journal j
SET revision = s.revision
FROM clacks.sync_accounts s
WHERE j.account = $1 AND s.account_id = j.account
AND j.revision > $2
