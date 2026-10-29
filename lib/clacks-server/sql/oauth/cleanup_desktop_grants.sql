DELETE FROM clacks.desktop_grants WHERE family_id IN (SELECT family_id FROM clacks.desktop_grants WHERE account_id=$1 AND state='provisional' AND provisional_until <= $2 LIMIT 500)
