CREATE TABLE clacks.service_credentials (
    account_id TEXT NOT NULL REFERENCES clacks.accounts,
    credential_id TEXT NOT NULL,
    purpose TEXT NOT NULL CHECK (purpose IN ('qrz.directory', 'hamqth.directory', 'qrz.logbook', 'lotw.account', 'lotw.signing')),
    material_revision BIGINT NOT NULL CHECK (material_revision > 0),
    state TEXT NOT NULL CHECK (state IN ('present', 'removed')),
    payload_format INTEGER,
    key_version BIGINT,
    nonce BYTEA,
    ciphertext BYTEA,
    created_at TIMESTAMPTZ NOT NULL,
    changed_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (account_id, credential_id),
    FOREIGN KEY (account_id, key_version) REFERENCES clacks.account_encryption_keys,
    CHECK ((state = 'present' AND payload_format IS NOT NULL AND payload_format > 0
        AND key_version IS NOT NULL AND nonce IS NOT NULL AND ciphertext IS NOT NULL)
        OR (state = 'removed' AND payload_format IS NULL AND key_version IS NULL
        AND nonce IS NULL AND ciphertext IS NULL))
);
CREATE INDEX service_credential_key ON clacks.service_credentials(account_id, key_version);
CREATE INDEX code_receipt_key ON clacks.desktop_authorization_codes(account_id, receipt_key_version)
    WHERE receipt_key_version IS NOT NULL;
