CREATE TABLE store_account (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    account TEXT NOT NULL UNIQUE CHECK (length(account) = 26),
    server TEXT,
    actor TEXT CHECK (actor IS NULL OR length(actor) = 26),
    credential_reference TEXT,
    CHECK ((server IS NULL AND actor IS NULL AND credential_reference IS NULL)
        OR (server IS NOT NULL AND length(server) > 0 AND actor IS NOT NULL
            AND credential_reference IS NOT NULL AND length(credential_reference) > 0))
) STRICT;

CREATE TABLE record_identity (
    account TEXT NOT NULL REFERENCES store_account(account),
    kind TEXT NOT NULL CHECK (kind IN ('qso', 'tag', 'profile', 'equipment', 'group', 'output_profile',
        'destination', 'imported', 'attempt', 'result', 'observation', 'association',
        'disposition', 'suppression', 'download', 'acquisition')),
    record_id TEXT NOT NULL CHECK (length(record_id) = 26),
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
