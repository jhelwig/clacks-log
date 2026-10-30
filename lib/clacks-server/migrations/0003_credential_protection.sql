CREATE INDEX code_receipt_key ON clacks.desktop_authorization_codes(account_id, receipt_key_version)
    WHERE receipt_key_version IS NOT NULL;
