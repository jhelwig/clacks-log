CREATE TABLE tag_payload (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'tag' CHECK (kind = 'tag'),
    record_id TEXT NOT NULL,
    name TEXT NOT NULL,
    lotw_policy TEXT NOT NULL CHECK (lotw_policy IN ('enable', 'forbid', 'neither')),
    qrz_policy TEXT NOT NULL CHECK (qrz_policy IN ('enable', 'forbid', 'neither')),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE profile_payload (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'profile' CHECK (kind = 'profile'),
    record_id TEXT NOT NULL,
    name TEXT NOT NULL,
    station_id INTEGER NOT NULL,
    equipment_kind TEXT NOT NULL DEFAULT 'equipment' CHECK (equipment_kind = 'equipment'),
    equipment_id TEXT,
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (station_id, account) REFERENCES station_snapshot(station_id, account),
    FOREIGN KEY (account, equipment_kind, equipment_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;

CREATE TABLE equipment_payload (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'equipment' CHECK (kind = 'equipment'),
    record_id TEXT NOT NULL,
    name TEXT NOT NULL,
    snapshot_id INTEGER NOT NULL,
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (snapshot_id, account) REFERENCES equipment_snapshot(equipment_id, account)
) STRICT;

CREATE TABLE output_profile_payload (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'output_profile' CHECK (kind = 'output_profile'),
    record_id TEXT NOT NULL,
    name TEXT NOT NULL,
    purpose TEXT NOT NULL,
    contextual_fields TEXT NOT NULL,
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE group_payload (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'group' CHECK (kind = 'group'),
    record_id TEXT NOT NULL,
    name TEXT NOT NULL,
    identifier_config TEXT NOT NULL,
    bears_location INTEGER NOT NULL CHECK (bears_location IN (0, 1)),
    catalog_binding TEXT,
    ordinary_bindings TEXT NOT NULL,
    UNIQUE (payload_id, account),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id) ON DELETE CASCADE
) STRICT;

CREATE TABLE export_plan (
    payload_id INTEGER NOT NULL,
    account TEXT NOT NULL,
    profile_kind TEXT NOT NULL DEFAULT 'output_profile' CHECK (profile_kind = 'output_profile'),
    profile_id TEXT NOT NULL,
    body TEXT NOT NULL,
    PRIMARY KEY (payload_id, profile_id),
    FOREIGN KEY (payload_id, account) REFERENCES group_payload(payload_id, account) ON DELETE CASCADE,
    FOREIGN KEY (account, profile_kind, profile_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
