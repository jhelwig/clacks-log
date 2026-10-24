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
    purpose TEXT NOT NULL CHECK (purpose IN ('ordinary', 'targeted')),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id) ON DELETE CASCADE
) STRICT;
CREATE TABLE output_context_field (
    payload_id INTEGER NOT NULL REFERENCES output_profile_payload(payload_id) ON DELETE CASCADE,
    field TEXT NOT NULL,
    PRIMARY KEY (payload_id, field)
) STRICT;

CREATE TABLE export_expression (
    expression_id INTEGER PRIMARY KEY,
    payload_id INTEGER NOT NULL REFERENCES group_payload(payload_id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    kind TEXT NOT NULL CHECK (kind IN ('literal', 'qso_field', 'current', 'identifiers', 'bound', 'capture', 'join')),
    text TEXT,
    side TEXT CHECK (side IN ('own', 'contacted')),
    child INTEGER,
    pattern TEXT,
    capture_name TEXT,
    UNIQUE (expression_id, payload_id),
    FOREIGN KEY (child, payload_id) REFERENCES export_expression(expression_id, payload_id),
    CHECK ((kind IN ('literal', 'qso_field', 'bound') AND text IS NOT NULL AND side IS NULL AND child IS NULL AND pattern IS NULL AND capture_name IS NULL)
        OR (kind = 'current' AND text IS NULL AND side IS NULL AND child IS NULL AND pattern IS NULL AND capture_name IS NULL)
        OR (kind = 'identifiers' AND text IS NULL AND side IS NOT NULL AND child IS NULL AND pattern IS NULL AND capture_name IS NULL)
        OR (kind = 'capture' AND text IS NULL AND side IS NULL AND child IS NOT NULL AND pattern IS NOT NULL AND capture_name IS NOT NULL)
        OR (kind = 'join' AND text IS NOT NULL AND side IS NULL AND child IS NOT NULL AND pattern IS NULL AND capture_name IS NULL))
) STRICT;

CREATE TABLE group_payload (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'group' CHECK (kind = 'group'),
    record_id TEXT NOT NULL,
    name TEXT NOT NULL,
    identifier_required INTEGER NOT NULL CHECK (identifier_required IN (0, 1)),
    syntax TEXT,
    bears_location INTEGER NOT NULL CHECK (bears_location IN (0, 1)),
    catalog_id TEXT,
    catalog_display TEXT,
    catalog_lookup INTEGER,
    UNIQUE (payload_id, account),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (catalog_lookup, payload_id) REFERENCES export_expression(expression_id, payload_id),
    CHECK ((catalog_id IS NULL AND catalog_display IS NULL AND catalog_lookup IS NULL)
        OR (catalog_id IS NOT NULL AND catalog_display IS NOT NULL AND catalog_lookup IS NOT NULL))
) STRICT;
CREATE TABLE group_normalization (
    payload_id INTEGER NOT NULL REFERENCES group_payload(payload_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    transformation TEXT NOT NULL CHECK (transformation IN ('trim', 'uppercase', 'lowercase')),
    PRIMARY KEY (payload_id, ordinal)
) STRICT;
CREATE TABLE catalog_display_detail (
    payload_id INTEGER NOT NULL REFERENCES group_payload(payload_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    label TEXT NOT NULL,
    field TEXT NOT NULL,
    PRIMARY KEY (payload_id, ordinal)
) STRICT;

CREATE TABLE export_plan (
    plan_id INTEGER PRIMARY KEY,
    payload_id INTEGER NOT NULL,
    account TEXT NOT NULL,
    profile_kind TEXT NOT NULL DEFAULT 'output_profile' CHECK (profile_kind = 'output_profile'),
    profile_id TEXT,
    UNIQUE (plan_id, payload_id),
    FOREIGN KEY (payload_id, account) REFERENCES group_payload(payload_id, account) ON DELETE CASCADE,
    FOREIGN KEY (account, profile_kind, profile_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
CREATE UNIQUE INDEX export_plan_ordinary ON export_plan(payload_id) WHERE profile_id IS NULL;
CREATE UNIQUE INDEX export_plan_target ON export_plan(payload_id, profile_id) WHERE profile_id IS NOT NULL;
CREATE TABLE export_field (
    plan_id INTEGER NOT NULL,
    payload_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    field TEXT NOT NULL,
    expression_id INTEGER NOT NULL,
    condition TEXT NOT NULL CHECK (condition IN ('always', 'bound', 'association')),
    binding TEXT,
    side TEXT CHECK (side IN ('own', 'contacted')),
    PRIMARY KEY (plan_id, ordinal),
    FOREIGN KEY (plan_id, payload_id) REFERENCES export_plan(plan_id, payload_id) ON DELETE CASCADE,
    FOREIGN KEY (expression_id, payload_id) REFERENCES export_expression(expression_id, payload_id),
    CHECK ((condition = 'always' AND binding IS NULL AND side IS NULL)
        OR (condition = 'bound' AND binding IS NOT NULL AND side IS NULL)
        OR (condition = 'association' AND binding IS NULL AND side IS NOT NULL))
) STRICT;
CREATE TABLE export_partition (
    plan_id INTEGER NOT NULL,
    payload_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    expression_id INTEGER,
    side TEXT CHECK (side IN ('own', 'contacted')),
    binding TEXT,
    PRIMARY KEY (plan_id, ordinal),
    FOREIGN KEY (plan_id, payload_id) REFERENCES export_plan(plan_id, payload_id) ON DELETE CASCADE,
    FOREIGN KEY (expression_id, payload_id) REFERENCES export_expression(expression_id, payload_id),
    CHECK ((expression_id IS NOT NULL AND side IS NULL AND binding IS NULL)
        OR (expression_id IS NULL AND side IS NOT NULL AND binding IS NOT NULL))
) STRICT;
CREATE TABLE export_expansion (
    plan_id INTEGER NOT NULL REFERENCES export_plan(plan_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    side TEXT NOT NULL CHECK (side IN ('own', 'contacted')),
    binding TEXT NOT NULL,
    keep_empty INTEGER NOT NULL CHECK (keep_empty IN (0, 1)),
    PRIMARY KEY (plan_id, ordinal)
) STRICT;
