CREATE TABLE clacks.directory_payload (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'directory' CHECK (kind = 'directory'),
    record_id TEXT NOT NULL,
    parent_kind TEXT NOT NULL DEFAULT 'qso' CHECK (parent_kind = 'qso'),
    UNIQUE (payload_id, account),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES clacks.payload(payload_id, account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (account, parent_kind, record_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.directory_snapshot (
    payload_id BIGINT NOT NULL,
    account TEXT NOT NULL,
    lookup_key TEXT NOT NULL,
    provider TEXT NOT NULL,
    lookup_seconds BIGINT NOT NULL,
    lookup_nanoseconds BIGINT NOT NULL,
    lookup_id TEXT NOT NULL,
    name TEXT,
    position_kind TEXT,
    locator TEXT,
    latitude BIGINT,
    longitude BIGINT,
    grid_length BIGINT,
    dxcc TEXT,
    primary_subdivision TEXT,
    secondary_subdivision TEXT,
    cq_zone BIGINT,
    itu_zone BIGINT,
    locality TEXT,
    returned_callsign TEXT,
    supplied_locator TEXT,
    supplied_latitude BIGINT,
    supplied_longitude BIGINT,
    coordinate_source TEXT,
    PRIMARY KEY (payload_id, lookup_key),
    UNIQUE (payload_id, lookup_key, account),
    FOREIGN KEY (payload_id, account) REFERENCES clacks.directory_payload(payload_id, account) ON DELETE CASCADE
);
CREATE INDEX directory_snapshot_key ON clacks.directory_snapshot(account, lookup_key, payload_id);

CREATE TABLE clacks.directory_reference (
    reference_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    payload_id BIGINT NOT NULL,
    account TEXT NOT NULL,
    lookup_key TEXT NOT NULL,
    group_kind TEXT NOT NULL DEFAULT 'group' CHECK (group_kind = 'group'),
    group_id TEXT NOT NULL,
    identifier TEXT,
    FOREIGN KEY (payload_id, lookup_key, account) REFERENCES clacks.directory_snapshot(payload_id, lookup_key, account) ON DELETE CASCADE,
    FOREIGN KEY (account, group_kind, group_id) REFERENCES clacks.record_identity(account, kind, record_id)
);
CREATE UNIQUE INDEX directory_reference_value ON clacks.directory_reference(payload_id, lookup_key, group_id, identifier) WHERE identifier IS NOT NULL;
CREATE UNIQUE INDEX directory_reference_group ON clacks.directory_reference(payload_id, lookup_key, group_id) WHERE identifier IS NULL;
