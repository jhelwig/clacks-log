CREATE TABLE store_account (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    account TEXT NOT NULL UNIQUE,
    server TEXT,
    actor TEXT,
    credential_reference TEXT
) STRICT;

CREATE TABLE record_identity (
    account TEXT NOT NULL REFERENCES store_account(account),
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    PRIMARY KEY (account, kind, record_id)
) STRICT;

CREATE TABLE payload (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    deleted INTEGER NOT NULL CHECK (deleted IN (0, 1)),
    UNIQUE (payload_id, account, kind, record_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
