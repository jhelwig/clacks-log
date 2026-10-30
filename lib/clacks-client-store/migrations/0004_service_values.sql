CREATE TABLE record_vector (
    vector_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    UNIQUE (vector_id, account, kind, record_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
CREATE TABLE vector_component (
    vector_id INTEGER NOT NULL REFERENCES record_vector(vector_id) ON DELETE CASCADE,
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('client', 'server')),
    actor_id TEXT NOT NULL CHECK (length(actor_id) = 26),
    counter INTEGER NOT NULL CHECK (counter > 0),
    PRIMARY KEY (vector_id, actor_kind, actor_id)
) STRICT;
CREATE TABLE version_stamp (
    stamp_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    timeline_id TEXT NOT NULL CHECK (length(timeline_id) = 26),
    revision INTEGER NOT NULL CHECK (revision > 0),
    baseline_id TEXT NOT NULL CHECK (length(baseline_id) = 26),
    vector_id INTEGER NOT NULL,
    UNIQUE (stamp_id, account, kind, record_id),
    FOREIGN KEY (vector_id, account, kind, record_id) REFERENCES record_vector(vector_id, account, kind, record_id)
) STRICT;

CREATE TABLE destination_descriptor (
    descriptor_id INTEGER PRIMARY KEY,
    payload_id INTEGER NOT NULL REFERENCES service_support(payload_id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    account TEXT NOT NULL,
    destination_kind TEXT NOT NULL DEFAULT 'destination' CHECK (destination_kind = 'destination'),
    destination_id TEXT NOT NULL,
    collection TEXT NOT NULL CHECK (collection IN ('lotw', 'qrz')),
    lotw_account TEXT,
    qrz_book TEXT,
    description TEXT NOT NULL,
    UNIQUE (descriptor_id, payload_id),
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES record_identity(account, kind, record_id),
    CHECK ((collection = 'lotw' AND lotw_account IS NOT NULL AND length(trim(lotw_account)) > 0 AND qrz_book IS NULL)
        OR (collection = 'qrz' AND lotw_account IS NULL)),
    CHECK (qrz_book IS NULL OR (length(qrz_book) > 0 AND qrz_book NOT GLOB '*[^0-9]*' AND substr(qrz_book, 1, 1) != '0'))
) STRICT;
CREATE TABLE external_key (
    external_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    destination_kind TEXT NOT NULL DEFAULT 'destination' CHECK (destination_kind = 'destination'),
    destination_id TEXT NOT NULL,
    canonical_key TEXT NOT NULL,
    UNIQUE (external_id, account),
    UNIQUE (account, destination_id, canonical_key),
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;

CREATE TABLE service_support (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('destination', 'imported', 'attempt', 'result', 'observation', 'association', 'disposition', 'suppression', 'download', 'acquisition')),
    record_id TEXT NOT NULL,
    operation_id TEXT CHECK (operation_id IS NULL OR length(operation_id) = 26),
    acquisition_stamp INTEGER,
    descriptor_id INTEGER,
    UNIQUE (payload_id, account, kind),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (acquisition_stamp, account, kind, record_id) REFERENCES version_stamp(stamp_id, account, kind, record_id),
    FOREIGN KEY (descriptor_id, payload_id) REFERENCES destination_descriptor(descriptor_id, payload_id)
) STRICT;
CREATE TABLE support_requirement (
    payload_id INTEGER NOT NULL REFERENCES service_support(payload_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    requirement TEXT NOT NULL CHECK (requirement IN ('fact', 'effect')),
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    stamp_id INTEGER NOT NULL,
    deleted INTEGER NOT NULL CHECK (deleted IN (0, 1)),
    PRIMARY KEY (payload_id, ordinal),
    FOREIGN KEY (stamp_id, account, kind, record_id) REFERENCES version_stamp(stamp_id, account, kind, record_id)
) STRICT;
CREATE TABLE service_destination (
    payload_id INTEGER PRIMARY KEY REFERENCES service_support(payload_id) ON DELETE CASCADE,
    descriptor_id INTEGER NOT NULL,
    FOREIGN KEY (descriptor_id, payload_id) REFERENCES destination_descriptor(descriptor_id, payload_id)
) STRICT;
CREATE TABLE service_association (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'association' CHECK (kind = 'association'),
    external_id INTEGER NOT NULL,
    qso_kind TEXT NOT NULL DEFAULT 'qso' CHECK (qso_kind = 'qso'),
    qso_id TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('active', 'superseded', 'deleted')),
    FOREIGN KEY (payload_id, account, kind) REFERENCES service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (external_id, account) REFERENCES external_key(external_id, account),
    FOREIGN KEY (account, qso_kind, qso_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
CREATE TABLE service_suppression (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'suppression' CHECK (kind = 'suppression'),
    external_id INTEGER NOT NULL,
    qso_kind TEXT NOT NULL DEFAULT 'qso' CHECK (qso_kind = 'qso'),
    qso_id TEXT NOT NULL,
    FOREIGN KEY (payload_id, account, kind) REFERENCES service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (external_id, account) REFERENCES external_key(external_id, account),
    FOREIGN KEY (account, qso_kind, qso_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
CREATE TABLE service_disposition (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'disposition' CHECK (kind = 'disposition'),
    observation_kind TEXT NOT NULL DEFAULT 'observation' CHECK (observation_kind = 'observation'),
    observation_id TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('review', 'associated', 'added', 'rejected', 'suppressed', 'equivalent')),
    association_kind TEXT NOT NULL DEFAULT 'association' CHECK (association_kind = 'association'),
    association_id TEXT,
    FOREIGN KEY (payload_id, account, kind) REFERENCES service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, observation_kind, observation_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (account, association_kind, association_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;

CREATE TABLE service_query (
    query_id INTEGER PRIMARY KEY,
    payload_id INTEGER NOT NULL REFERENCES service_support(payload_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    destination_kind TEXT NOT NULL DEFAULT 'destination' CHECK (destination_kind = 'destination'),
    destination_id TEXT NOT NULL,
    stream TEXT NOT NULL CHECK (stream IN ('receipt', 'confirmation', 'qrz')),
    own_call TEXT,
    since INTEGER,
    selected_fields TEXT NOT NULL,
    UNIQUE (query_id, payload_id),
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
CREATE TABLE service_download (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'download' CHECK (kind = 'download'),
    destination_kind TEXT NOT NULL DEFAULT 'destination' CHECK (destination_kind = 'destination'),
    destination_id TEXT NOT NULL,
    acquisition_kind TEXT NOT NULL DEFAULT 'acquisition' CHECK (acquisition_kind = 'acquisition'),
    active_id TEXT,
    FOREIGN KEY (payload_id, account, kind) REFERENCES service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (account, acquisition_kind, active_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
CREATE TABLE stream_progress (
    payload_id INTEGER NOT NULL REFERENCES service_download(payload_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    query_id INTEGER NOT NULL,
    watermark INTEGER,
    PRIMARY KEY (payload_id, ordinal),
    FOREIGN KEY (query_id, payload_id) REFERENCES service_query(query_id, payload_id)
) STRICT;
CREATE TABLE service_acquisition (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'acquisition' CHECK (kind = 'acquisition'),
    query_id INTEGER NOT NULL,
    phase TEXT NOT NULL CHECK (phase IN ('active', 'completed', 'failed')),
    failure TEXT,
    started_seconds INTEGER NOT NULL,
    started_nanos INTEGER NOT NULL CHECK (started_nanos BETWEEN 0 AND 999999999),
    finished_seconds INTEGER,
    finished_nanos INTEGER CHECK (finished_nanos BETWEEN 0 AND 999999999),
    complete INTEGER NOT NULL CHECK (complete IN (0, 1)),
    FOREIGN KEY (payload_id, account, kind) REFERENCES service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (query_id, payload_id) REFERENCES service_query(query_id, payload_id),
    CHECK ((phase = 'failed' AND failure IS NOT NULL) OR (phase != 'failed' AND failure IS NULL)),
    CHECK ((phase = 'active' AND finished_seconds IS NULL AND finished_nanos IS NULL)
        OR (phase != 'active' AND finished_seconds IS NOT NULL AND finished_nanos IS NOT NULL)),
    CHECK (complete = 0 OR phase = 'completed')
) STRICT;
CREATE TABLE acquisition_input (
    payload_id INTEGER NOT NULL REFERENCES service_acquisition(payload_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    observation_kind TEXT NOT NULL DEFAULT 'observation' CHECK (observation_kind = 'observation'),
    observation_id TEXT NOT NULL,
    PRIMARY KEY (payload_id, observation_id),
    FOREIGN KEY (account, observation_kind, observation_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;

CREATE TABLE service_field_map (
    map_id INTEGER PRIMARY KEY,
    payload_id INTEGER NOT NULL REFERENCES service_support(payload_id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    body TEXT NOT NULL,
    UNIQUE (map_id, payload_id),
    UNIQUE (payload_id, role)
) STRICT;
CREATE TABLE submission_projection (
    projection_id INTEGER PRIMARY KEY,
    payload_id INTEGER NOT NULL REFERENCES service_support(payload_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    destination_kind TEXT NOT NULL DEFAULT 'destination' CHECK (destination_kind = 'destination'),
    destination_id TEXT NOT NULL,
    service TEXT NOT NULL CHECK (service IN ('lotw', 'qrz')),
    mapping_release TEXT,
    mapping_commit TEXT,
    signing_call TEXT,
    signing_dxcc TEXT,
    external_id INTEGER,
    contact_map INTEGER NOT NULL,
    station_map INTEGER,
    UNIQUE (projection_id, payload_id),
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (external_id, account) REFERENCES external_key(external_id, account),
    FOREIGN KEY (contact_map, payload_id) REFERENCES service_field_map(map_id, payload_id),
    FOREIGN KEY (station_map, payload_id) REFERENCES service_field_map(map_id, payload_id),
    CHECK ((service = 'lotw' AND mapping_release IS NOT NULL AND mapping_commit IS NOT NULL AND signing_call IS NOT NULL AND signing_dxcc IS NOT NULL AND external_id IS NOT NULL AND station_map IS NOT NULL)
        OR (service = 'qrz' AND mapping_release IS NULL AND mapping_commit IS NULL AND signing_call IS NULL AND signing_dxcc IS NULL AND external_id IS NULL AND station_map IS NULL))
) STRICT;
CREATE TABLE service_attempt (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'attempt' CHECK (kind = 'attempt'),
    qso_kind TEXT NOT NULL DEFAULT 'qso' CHECK (qso_kind = 'qso'),
    qso_id TEXT NOT NULL,
    projection_id INTEGER NOT NULL,
    phase TEXT NOT NULL CHECK (phase IN ('intent', 'possible', 'dispatched')),
    at_seconds INTEGER NOT NULL,
    at_nanos INTEGER NOT NULL CHECK (at_nanos BETWEEN 0 AND 999999999),
    batch_id TEXT CHECK (batch_id IS NULL OR length(batch_id) = 26),
    FOREIGN KEY (payload_id, account, kind) REFERENCES service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, qso_kind, qso_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (projection_id, payload_id) REFERENCES submission_projection(projection_id, payload_id)
) STRICT;
CREATE TABLE service_result (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'result' CHECK (kind = 'result'),
    destination_kind TEXT NOT NULL DEFAULT 'destination' CHECK (destination_kind = 'destination'),
    destination_id TEXT NOT NULL,
    scope TEXT NOT NULL CHECK (scope IN ('individual', 'batch')),
    batch_id TEXT,
    outcome TEXT NOT NULL CHECK (outcome IN ('success', 'rejected', 'uncertain')),
    outcome_detail TEXT,
    phase TEXT NOT NULL CHECK (phase IN ('intent', 'possible', 'dispatched')),
    at_seconds INTEGER NOT NULL,
    at_nanos INTEGER NOT NULL CHECK (at_nanos BETWEEN 0 AND 999999999),
    diagnostic TEXT,
    external_id INTEGER,
    attempt_kind TEXT NOT NULL DEFAULT 'attempt' CHECK (attempt_kind = 'attempt'),
    external_attempt TEXT,
    FOREIGN KEY (payload_id, account, kind) REFERENCES service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (external_id, account) REFERENCES external_key(external_id, account),
    FOREIGN KEY (account, attempt_kind, external_attempt) REFERENCES record_identity(account, kind, record_id),
    CHECK ((scope = 'individual' AND batch_id IS NULL) OR (scope = 'batch' AND batch_id IS NOT NULL AND length(batch_id) = 26)),
    CHECK ((outcome = 'success' AND outcome_detail IS NULL) OR (outcome != 'success' AND outcome_detail IS NOT NULL)),
    CHECK (external_attempt IS NULL OR external_id IS NOT NULL)
) STRICT;
CREATE TABLE result_member (
    payload_id INTEGER NOT NULL REFERENCES service_result(payload_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    attempt_kind TEXT NOT NULL DEFAULT 'attempt' CHECK (attempt_kind = 'attempt'),
    attempt_id TEXT NOT NULL,
    PRIMARY KEY (payload_id, attempt_id),
    FOREIGN KEY (account, attempt_kind, attempt_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;

CREATE TABLE contact_report (
    report_id INTEGER PRIMARY KEY,
    payload_id INTEGER NOT NULL REFERENCES service_support(payload_id) ON DELETE CASCADE,
    own_call_state INTEGER NOT NULL CHECK (own_call_state IN (0,1,2)),
    own_call TEXT,
    worked_call_state INTEGER NOT NULL CHECK (worked_call_state IN (0,1,2)),
    worked_call TEXT,
    start_state INTEGER NOT NULL CHECK (start_state IN (0,1,2)),
    start_kind TEXT CHECK (start_kind IN ('second','minute')),
    start INTEGER,
    band_state INTEGER NOT NULL CHECK (band_state IN (0,1,2)),
    band TEXT,
    receive_band_state INTEGER NOT NULL CHECK (receive_band_state IN (0,1,2)),
    receive_band TEXT,
    frequency_state INTEGER NOT NULL CHECK (frequency_state IN (0,1,2)),
    frequency INTEGER CHECK (frequency > 0),
    receive_frequency_state INTEGER NOT NULL CHECK (receive_frequency_state IN (0,1,2)),
    receive_frequency INTEGER CHECK (receive_frequency > 0),
    mode_state INTEGER NOT NULL CHECK (mode_state IN (0,1,2)),
    mode TEXT,
    mode_parent TEXT,
    mode_group TEXT,
    confirmation_state INTEGER NOT NULL CHECK (confirmation_state IN (0,1,2)),
    confirmation TEXT CHECK (confirmation IN ('confirmed','unconfirmed')),
    UNIQUE (report_id, payload_id),
    CHECK ((own_call_state = 2) = (own_call IS NOT NULL)),
    CHECK ((worked_call_state = 2) = (worked_call IS NOT NULL)),
    CHECK ((start_state = 2 AND start_kind IS NOT NULL AND start IS NOT NULL) OR (start_state != 2 AND start_kind IS NULL AND start IS NULL)),
    CHECK ((band_state = 2) = (band IS NOT NULL)),
    CHECK ((receive_band_state = 2) = (receive_band IS NOT NULL)),
    CHECK ((frequency_state = 2) = (frequency IS NOT NULL)),
    CHECK ((receive_frequency_state = 2) = (receive_frequency IS NOT NULL)),
    CHECK ((mode_state = 2 AND mode IS NOT NULL) OR (mode_state != 2 AND mode IS NULL AND mode_parent IS NULL AND mode_group IS NULL)),
    CHECK ((confirmation_state = 2) = (confirmation IS NOT NULL))
) STRICT;
CREATE TABLE station_report (
    report_id INTEGER PRIMARY KEY,
    payload_id INTEGER NOT NULL REFERENCES service_support(payload_id) ON DELETE CASCADE,
    name_state INTEGER NOT NULL CHECK (name_state IN (0,1,2)),
    name TEXT,
    position_state INTEGER NOT NULL CHECK (position_state IN (0,1,2)),
    position_kind TEXT CHECK (position_kind IN ('grid','point')),
    locator TEXT,
    latitude INTEGER CHECK (latitude BETWEEN -90000000 AND 90000000),
    longitude INTEGER CHECK (longitude BETWEEN -180000000 AND 180000000),
    grid_length INTEGER CHECK (grid_length IN (2,4,6,8,10,12)),
    dxcc_state INTEGER NOT NULL CHECK (dxcc_state IN (0,1,2)),
    dxcc TEXT,
    primary_state INTEGER NOT NULL CHECK (primary_state IN (0,1,2)),
    primary_subdivision TEXT,
    secondary_state INTEGER NOT NULL CHECK (secondary_state IN (0,1,2)),
    secondary_subdivision TEXT,
    cq_state INTEGER NOT NULL CHECK (cq_state IN (0,1,2)),
    cq_zone INTEGER CHECK (cq_zone BETWEEN 1 AND 40),
    itu_state INTEGER NOT NULL CHECK (itu_state IN (0,1,2)),
    itu_zone INTEGER CHECK (itu_zone BETWEEN 1 AND 90),
    references_state INTEGER NOT NULL CHECK (references_state IN (0,1,2)),
    UNIQUE (report_id, payload_id),
    CHECK ((name_state = 2) = (name IS NOT NULL)),
    CHECK ((dxcc_state = 2) = (dxcc IS NOT NULL)),
    CHECK ((primary_state = 2) = (primary_subdivision IS NOT NULL)),
    CHECK ((secondary_state = 2) = (secondary_subdivision IS NOT NULL)),
    CHECK ((cq_state = 2) = (cq_zone IS NOT NULL)),
    CHECK ((itu_state = 2) = (itu_zone IS NOT NULL)),
    CHECK ((position_state != 2 AND position_kind IS NULL AND locator IS NULL AND latitude IS NULL AND longitude IS NULL AND grid_length IS NULL)
        OR (position_state = 2 AND position_kind IS NOT NULL AND position_kind = 'grid' AND locator IS NOT NULL AND latitude IS NULL AND longitude IS NULL AND grid_length IS NULL)
        OR (position_state = 2 AND position_kind IS NOT NULL AND position_kind = 'point' AND locator IS NULL AND latitude IS NOT NULL AND longitude IS NOT NULL AND grid_length IS NOT NULL))
) STRICT;
CREATE TABLE reported_reference (
    reference_id INTEGER PRIMARY KEY,
    report_id INTEGER NOT NULL REFERENCES station_report(report_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    group_kind TEXT NOT NULL DEFAULT 'group' CHECK (group_kind = 'group'),
    group_id TEXT NOT NULL,
    identifier TEXT,
    FOREIGN KEY (account, group_kind, group_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
CREATE UNIQUE INDEX reported_reference_value ON reported_reference(report_id, group_id, identifier) WHERE identifier IS NOT NULL;
CREATE UNIQUE INDEX reported_reference_group ON reported_reference(report_id, group_id) WHERE identifier IS NULL;

CREATE TABLE service_observation (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'observation' CHECK (kind = 'observation'),
    destination_kind TEXT NOT NULL DEFAULT 'destination' CHECK (destination_kind = 'destination'),
    destination_id TEXT NOT NULL,
    identity TEXT NOT NULL CHECK (identity IN ('complete','incomplete')),
    external_id INTEGER,
    incomplete_service TEXT CHECK (incomplete_service IN ('lotw','qrz')),
    available_map INTEGER,
    contact_report INTEGER NOT NULL,
    own_report INTEGER NOT NULL,
    counterpart_report INTEGER,
    qso_order INTEGER,
    confirmation_order INTEGER,
    acquisition_kind TEXT NOT NULL DEFAULT 'acquisition' CHECK (acquisition_kind = 'acquisition'),
    acquisition_id TEXT NOT NULL,
    query_id INTEGER NOT NULL,
    observed_seconds INTEGER NOT NULL,
    observed_nanos INTEGER NOT NULL CHECK (observed_nanos BETWEEN 0 AND 999999999),
    FOREIGN KEY (payload_id, account, kind) REFERENCES service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (external_id, account) REFERENCES external_key(external_id, account),
    FOREIGN KEY (available_map, payload_id) REFERENCES service_field_map(map_id, payload_id),
    FOREIGN KEY (contact_report, payload_id) REFERENCES contact_report(report_id, payload_id),
    FOREIGN KEY (own_report, payload_id) REFERENCES station_report(report_id, payload_id),
    FOREIGN KEY (counterpart_report, payload_id) REFERENCES station_report(report_id, payload_id),
    FOREIGN KEY (account, acquisition_kind, acquisition_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (query_id, payload_id) REFERENCES service_query(query_id, payload_id),
    CHECK ((identity = 'complete' AND external_id IS NOT NULL AND incomplete_service IS NULL AND available_map IS NULL)
        OR (identity = 'incomplete' AND external_id IS NULL AND incomplete_service IS NOT NULL AND available_map IS NOT NULL))
) STRICT;

CREATE TABLE imported_location (
    payload_id INTEGER PRIMARY KEY REFERENCES service_assertion(payload_id) ON DELETE CASCADE,
    position_kind TEXT CHECK (position_kind IN ('grid','point')),
    locator TEXT,
    latitude INTEGER CHECK (latitude BETWEEN -90000000 AND 90000000),
    longitude INTEGER CHECK (longitude BETWEEN -180000000 AND 180000000),
    grid_length INTEGER CHECK (grid_length IN (2,4,6,8,10,12)),
    dxcc TEXT,
    primary_subdivision TEXT,
    secondary_subdivision TEXT,
    cq_zone INTEGER CHECK (cq_zone BETWEEN 1 AND 40),
    itu_zone INTEGER CHECK (itu_zone BETWEEN 1 AND 90),
    CHECK ((position_kind IS NULL AND locator IS NULL AND latitude IS NULL AND longitude IS NULL AND grid_length IS NULL)
        OR (position_kind IS NOT NULL AND position_kind = 'grid' AND locator IS NOT NULL AND latitude IS NULL AND longitude IS NULL AND grid_length IS NULL)
        OR (position_kind IS NOT NULL AND position_kind = 'point' AND locator IS NULL AND latitude IS NOT NULL AND longitude IS NOT NULL AND grid_length IS NOT NULL))
) STRICT;
CREATE TABLE confirmation_basis (
    payload_id INTEGER PRIMARY KEY REFERENCES service_assertion(payload_id) ON DELETE CASCADE,
    uninterpreted_band TEXT,
    uninterpreted_receive_band TEXT,
    own_call TEXT,
    worked_call TEXT,
    start INTEGER,
    band TEXT,
    receive_band TEXT,
    frequency INTEGER CHECK (frequency > 0),
    receive_frequency INTEGER CHECK (receive_frequency > 0),
    mode TEXT,
    mode_parent TEXT,
    mode_group TEXT,
    CHECK (mode IS NOT NULL OR (mode_parent IS NULL AND mode_group IS NULL))
) STRICT;
CREATE TABLE service_assertion (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'imported' CHECK (kind = 'imported'),
    qso_kind TEXT NOT NULL DEFAULT 'qso' CHECK (qso_kind = 'qso'),
    qso_id TEXT NOT NULL,
    service TEXT NOT NULL CHECK (service IN ('lotw','qrz')),
    destination_kind TEXT NOT NULL DEFAULT 'destination' CHECK (destination_kind = 'destination'),
    destination_id TEXT,
    claim TEXT NOT NULL CHECK (claim IN ('uploaded','confirmed')),
    date INTEGER,
    source TEXT NOT NULL,
    source_record TEXT NOT NULL,
    mapping TEXT NOT NULL,
    native_end INTEGER,
    fields_map INTEGER NOT NULL,
    coverage TEXT NOT NULL,
    has_location INTEGER NOT NULL CHECK (has_location IN (0,1)),
    FOREIGN KEY (payload_id, account, kind) REFERENCES service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, qso_kind, qso_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (fields_map, payload_id) REFERENCES service_field_map(map_id, payload_id)
) STRICT;
