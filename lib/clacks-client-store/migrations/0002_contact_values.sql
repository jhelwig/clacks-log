CREATE TABLE station_snapshot (
    station_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL REFERENCES store_account(account),
    callsign TEXT NOT NULL CHECK (length(trim(callsign)) > 0),
    UNIQUE (station_id, account)
) STRICT;

CREATE TABLE recorded_location (
    station_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    position_kind TEXT CHECK (position_kind IN ('grid', 'point')),
    locator TEXT,
    latitude INTEGER CHECK (latitude BETWEEN -90000000 AND 90000000),
    longitude INTEGER CHECK (longitude BETWEEN -180000000 AND 180000000),
    grid_length INTEGER CHECK (grid_length IN (2, 4, 6, 8, 10, 12)),
    dxcc TEXT,
    primary_subdivision TEXT,
    secondary_subdivision TEXT,
    cq_zone INTEGER CHECK (cq_zone BETWEEN 1 AND 40),
    itu_zone INTEGER CHECK (itu_zone BETWEEN 1 AND 90),
    FOREIGN KEY (station_id, account) REFERENCES station_snapshot(station_id, account) ON DELETE CASCADE,
    CHECK ((position_kind IS NULL AND locator IS NULL AND latitude IS NULL AND longitude IS NULL AND grid_length IS NULL)
        OR (position_kind IS NOT NULL AND position_kind = 'grid' AND locator IS NOT NULL AND latitude IS NULL AND longitude IS NULL AND grid_length IS NULL)
        OR (position_kind IS NOT NULL AND position_kind = 'point' AND locator IS NULL AND latitude IS NOT NULL AND longitude IS NOT NULL AND grid_length IS NOT NULL))
) STRICT;

CREATE TABLE station_reference (
    reference_id INTEGER PRIMARY KEY,
    station_id INTEGER NOT NULL,
    account TEXT NOT NULL,
    group_kind TEXT NOT NULL DEFAULT 'group' CHECK (group_kind = 'group'),
    group_id TEXT NOT NULL,
    identifier TEXT CHECK (identifier IS NULL OR length(trim(identifier)) > 0),
    FOREIGN KEY (station_id, account) REFERENCES station_snapshot(station_id, account) ON DELETE CASCADE,
    FOREIGN KEY (account, group_kind, group_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
CREATE UNIQUE INDEX station_reference_value ON station_reference(station_id, group_id, identifier) WHERE identifier IS NOT NULL;
CREATE UNIQUE INDEX station_reference_group ON station_reference(station_id, group_id) WHERE identifier IS NULL;

CREATE TABLE equipment_snapshot (
    equipment_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL REFERENCES store_account(account),
    equipment_description TEXT,
    antenna_description TEXT,
    notes TEXT,
    UNIQUE (equipment_id, account)
) STRICT;

CREATE TABLE qso_payload (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'qso' CHECK (kind = 'qso'),
    record_id TEXT NOT NULL,
    start INTEGER NOT NULL,
    end INTEGER CHECK (end IS NULL OR end >= start),
    own_station INTEGER NOT NULL,
    contacted_station INTEGER NOT NULL,
    own_operator_call TEXT,
    own_operator_name TEXT,
    contacted_operator_call TEXT,
    contacted_operator_name TEXT,
    power INTEGER CHECK (power > 0),
    equipment_id INTEGER,
    frequency INTEGER CHECK (frequency > 0),
    band TEXT CHECK (band IS NULL OR length(trim(band)) > 0),
    receive_frequency INTEGER CHECK (receive_frequency > 0),
    receive_band TEXT CHECK (receive_band IS NULL OR length(trim(receive_band)) > 0),
    mode_kind TEXT NOT NULL CHECK (mode_kind IN ('parent', 'specialization')),
    mode_label TEXT NOT NULL CHECK (length(trim(mode_label)) > 0),
    mode_parent TEXT CHECK (mode_parent IS NULL OR length(trim(mode_parent)) > 0),
    sent_report TEXT,
    received_report TEXT,
    comment TEXT,
    UNIQUE (payload_id, account),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (own_station, account) REFERENCES station_snapshot(station_id, account),
    FOREIGN KEY (contacted_station, account) REFERENCES station_snapshot(station_id, account),
    FOREIGN KEY (equipment_id, account) REFERENCES equipment_snapshot(equipment_id, account),
    CHECK (frequency IS NOT NULL OR band IS NOT NULL),
    CHECK (mode_kind != 'parent' OR mode_parent IS NULL)
) STRICT;

CREATE TABLE qso_tag (
    payload_id INTEGER NOT NULL,
    account TEXT NOT NULL,
    tag_kind TEXT NOT NULL DEFAULT 'tag' CHECK (tag_kind = 'tag'),
    tag_id TEXT NOT NULL,
    PRIMARY KEY (payload_id, tag_id),
    FOREIGN KEY (payload_id, account) REFERENCES qso_payload(payload_id, account) ON DELETE CASCADE,
    FOREIGN KEY (account, tag_kind, tag_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
