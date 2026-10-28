SELECT EXISTS (SELECT 1 FROM clacks.server_administrators WHERE account_id = $1) AS "present!"
