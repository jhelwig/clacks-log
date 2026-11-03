CREATE TABLE clacks.directory_configuration (
    account_id TEXT PRIMARY KEY REFERENCES clacks.accounts(account_id),
    revision BIGINT NOT NULL,
    selected_provider TEXT
);
CREATE TABLE clacks.directory_material (
    account_id TEXT NOT NULL REFERENCES clacks.accounts(account_id),
    provider TEXT NOT NULL,
    material_id TEXT NOT NULL,
    material_revision BIGINT NOT NULL,
    encrypted_envelope TEXT NOT NULL,
    account_key_version BIGINT GENERATED ALWAYS AS (NULLIF(split_part(encrypted_envelope, '$', 5), '')::BIGINT) STORED NOT NULL,
    PRIMARY KEY (account_id, provider),
    FOREIGN KEY (account_id, account_key_version) REFERENCES clacks.account_encryption_keys(account_id, key_version)
);
CREATE INDEX directory_material_key ON clacks.directory_material(account_id, account_key_version);
