SELECT receipt_key_version AS "key_version!", receipt_nonce AS "nonce!",
       receipt_ciphertext AS "ciphertext!"
FROM clacks.desktop_authorization_codes WHERE code_digest = $1
