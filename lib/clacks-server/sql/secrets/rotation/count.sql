SELECT ((SELECT count(*) FROM clacks.desktop_authorization_codes WHERE account_id = $1 AND receipt_key_version = $2)
      + (SELECT count(*) FROM clacks.directory_material WHERE account_id = $1 AND account_key_version = $2)) AS "total!"
