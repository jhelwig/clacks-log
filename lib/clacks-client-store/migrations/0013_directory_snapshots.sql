CREATE TABLE directory_payload (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'directory' CHECK (kind = 'directory'),
    record_id TEXT NOT NULL,
    parent_kind TEXT NOT NULL DEFAULT 'qso' CHECK (parent_kind = 'qso'),
    UNIQUE (payload_id, account),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (account, parent_kind, record_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;

CREATE TABLE directory_snapshot (
    payload_id INTEGER NOT NULL,
    account TEXT NOT NULL,
    lookup_key TEXT NOT NULL,
    provider TEXT NOT NULL,
    lookup_seconds INTEGER NOT NULL,
    lookup_nanoseconds INTEGER NOT NULL,
    lookup_id TEXT NOT NULL,
    name TEXT,
    position_kind TEXT,
    locator TEXT,
    latitude INTEGER,
    longitude INTEGER,
    grid_length INTEGER,
    dxcc TEXT,
    primary_subdivision TEXT,
    secondary_subdivision TEXT,
    cq_zone INTEGER,
    itu_zone INTEGER,
    locality TEXT,
    returned_callsign TEXT,
    supplied_locator TEXT,
    supplied_latitude INTEGER,
    supplied_longitude INTEGER,
    coordinate_source TEXT,
    PRIMARY KEY (payload_id, lookup_key),
    UNIQUE (payload_id, lookup_key, account),
    FOREIGN KEY (payload_id, account) REFERENCES directory_payload(payload_id, account) ON DELETE CASCADE
) STRICT;
CREATE INDEX directory_snapshot_key ON directory_snapshot(account, lookup_key, payload_id);

CREATE TABLE directory_reference (
    reference_id INTEGER PRIMARY KEY,
    payload_id INTEGER NOT NULL,
    account TEXT NOT NULL,
    lookup_key TEXT NOT NULL,
    group_kind TEXT NOT NULL DEFAULT 'group' CHECK (group_kind = 'group'),
    group_id TEXT NOT NULL,
    identifier TEXT,
    FOREIGN KEY (payload_id, lookup_key, account) REFERENCES directory_snapshot(payload_id, lookup_key, account) ON DELETE CASCADE,
    FOREIGN KEY (account, group_kind, group_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
CREATE UNIQUE INDEX directory_reference_value ON directory_reference(payload_id, lookup_key, group_id, identifier) WHERE identifier IS NOT NULL;
CREATE UNIQUE INDEX directory_reference_group ON directory_reference(payload_id, lookup_key, group_id) WHERE identifier IS NULL;
