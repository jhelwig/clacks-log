CREATE TABLE record_vector (
    vector_id INTEGER PRIMARY KEY,
    body TEXT NOT NULL,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    UNIQUE (vector_id, account, kind, record_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
CREATE TABLE version_stamp (
    stamp_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    timeline_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    baseline_id TEXT NOT NULL,
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
    collection TEXT NOT NULL,
    lotw_account TEXT,
    qrz_book TEXT,
    description TEXT NOT NULL,
    UNIQUE (descriptor_id, payload_id),
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES record_identity(account, kind, record_id)
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
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    operation_id TEXT,
    acquisition_stamp INTEGER,
    descriptor_id INTEGER,
    UNIQUE (payload_id, account, kind),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (acquisition_stamp, account, kind, record_id) REFERENCES version_stamp(stamp_id, account, kind, record_id),
    FOREIGN KEY (descriptor_id, payload_id) REFERENCES destination_descriptor(descriptor_id, payload_id)
) STRICT;
CREATE TABLE support_requirement (
    payload_id INTEGER NOT NULL REFERENCES service_support(payload_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    requirement TEXT NOT NULL,
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
    state TEXT NOT NULL,
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
    decision TEXT NOT NULL,
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
    stream TEXT NOT NULL,
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
    ordinal INTEGER NOT NULL,
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
    phase TEXT NOT NULL,
    failure TEXT,
    started_seconds INTEGER NOT NULL,
    started_nanos INTEGER NOT NULL,
    finished_seconds INTEGER,
    finished_nanos INTEGER,
    complete INTEGER NOT NULL CHECK (complete IN (0, 1)),
    FOREIGN KEY (payload_id, account, kind) REFERENCES service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (query_id, payload_id) REFERENCES service_query(query_id, payload_id)
) STRICT;
CREATE TABLE acquisition_input (
    payload_id INTEGER NOT NULL REFERENCES service_acquisition(payload_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    observation_kind TEXT NOT NULL DEFAULT 'observation' CHECK (observation_kind = 'observation'),
    observation_id TEXT NOT NULL,
    PRIMARY KEY (payload_id, observation_id),
    FOREIGN KEY (account, observation_kind, observation_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;

CREATE TABLE submission_projection (
    projection_id INTEGER PRIMARY KEY,
    payload_id INTEGER NOT NULL REFERENCES service_support(payload_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    destination_kind TEXT NOT NULL DEFAULT 'destination' CHECK (destination_kind = 'destination'),
    destination_id TEXT NOT NULL,
    service TEXT NOT NULL,
    mapping_release TEXT,
    mapping_commit TEXT,
    signing_call TEXT,
    signing_dxcc TEXT,
    external_id INTEGER,
    contact_fields TEXT NOT NULL,
    station_fields TEXT,
    UNIQUE (projection_id, payload_id),
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (external_id, account) REFERENCES external_key(external_id, account)
) STRICT;
CREATE TABLE service_attempt (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'attempt' CHECK (kind = 'attempt'),
    qso_kind TEXT NOT NULL DEFAULT 'qso' CHECK (qso_kind = 'qso'),
    qso_id TEXT NOT NULL,
    projection_id INTEGER NOT NULL,
    phase TEXT NOT NULL,
    at_seconds INTEGER NOT NULL,
    at_nanos INTEGER NOT NULL,
    batch_id TEXT,
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
    scope TEXT NOT NULL,
    batch_id TEXT,
    outcome TEXT NOT NULL,
    outcome_detail TEXT,
    phase TEXT NOT NULL,
    at_seconds INTEGER NOT NULL,
    at_nanos INTEGER NOT NULL,
    diagnostic TEXT,
    external_id INTEGER,
    attempt_kind TEXT NOT NULL DEFAULT 'attempt' CHECK (attempt_kind = 'attempt'),
    external_attempt TEXT,
    FOREIGN KEY (payload_id, account, kind) REFERENCES service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (external_id, account) REFERENCES external_key(external_id, account),
    FOREIGN KEY (account, attempt_kind, external_attempt) REFERENCES record_identity(account, kind, record_id)
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
    own_call_state INTEGER NOT NULL,
    own_call TEXT,
    worked_call_state INTEGER NOT NULL,
    worked_call TEXT,
    start_state INTEGER NOT NULL,
    start_kind TEXT,
    start INTEGER,
    band_state INTEGER NOT NULL,
    band TEXT,
    receive_band_state INTEGER NOT NULL,
    receive_band TEXT,
    frequency_state INTEGER NOT NULL,
    frequency INTEGER,
    receive_frequency_state INTEGER NOT NULL,
    receive_frequency INTEGER,
    mode_state INTEGER NOT NULL,
    mode TEXT,
    mode_parent TEXT,
    mode_group TEXT,
    confirmation_state INTEGER NOT NULL,
    confirmation TEXT,
    UNIQUE (report_id, payload_id)
) STRICT;
CREATE TABLE station_report (
    report_id INTEGER PRIMARY KEY,
    payload_id INTEGER NOT NULL REFERENCES service_support(payload_id) ON DELETE CASCADE,
    name_state INTEGER NOT NULL,
    name TEXT,
    position_state INTEGER NOT NULL,
    position_kind TEXT,
    locator TEXT,
    latitude INTEGER,
    longitude INTEGER,
    grid_length INTEGER,
    dxcc_state INTEGER NOT NULL,
    dxcc TEXT,
    primary_state INTEGER NOT NULL,
    primary_subdivision TEXT,
    secondary_state INTEGER NOT NULL,
    secondary_subdivision TEXT,
    cq_state INTEGER NOT NULL,
    cq_zone INTEGER,
    itu_state INTEGER NOT NULL,
    itu_zone INTEGER,
    references_state INTEGER NOT NULL,
    UNIQUE (report_id, payload_id)
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
    identity TEXT NOT NULL,
    external_id INTEGER,
    incomplete_service TEXT,
    available_fields TEXT,
    contact_report INTEGER NOT NULL,
    own_report INTEGER NOT NULL,
    counterpart_report INTEGER,
    qso_order INTEGER,
    confirmation_order INTEGER,
    acquisition_kind TEXT NOT NULL DEFAULT 'acquisition' CHECK (acquisition_kind = 'acquisition'),
    acquisition_id TEXT NOT NULL,
    query_id INTEGER NOT NULL,
    observed_seconds INTEGER NOT NULL,
    observed_nanos INTEGER NOT NULL,
    FOREIGN KEY (payload_id, account, kind) REFERENCES service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (external_id, account) REFERENCES external_key(external_id, account),
    FOREIGN KEY (contact_report, payload_id) REFERENCES contact_report(report_id, payload_id),
    FOREIGN KEY (own_report, payload_id) REFERENCES station_report(report_id, payload_id),
    FOREIGN KEY (counterpart_report, payload_id) REFERENCES station_report(report_id, payload_id),
    FOREIGN KEY (account, acquisition_kind, acquisition_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (query_id, payload_id) REFERENCES service_query(query_id, payload_id)
) STRICT;

CREATE TABLE imported_location (
    payload_id INTEGER PRIMARY KEY REFERENCES service_assertion(payload_id) ON DELETE CASCADE,
    position_kind TEXT,
    locator TEXT,
    latitude INTEGER,
    longitude INTEGER,
    grid_length INTEGER,
    dxcc TEXT,
    primary_subdivision TEXT,
    secondary_subdivision TEXT,
    cq_zone INTEGER,
    itu_zone INTEGER
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
    frequency INTEGER,
    receive_frequency INTEGER,
    mode TEXT,
    mode_parent TEXT,
    mode_group TEXT
) STRICT;
CREATE TABLE service_assertion (
    payload_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'imported' CHECK (kind = 'imported'),
    qso_kind TEXT NOT NULL DEFAULT 'qso' CHECK (qso_kind = 'qso'),
    qso_id TEXT NOT NULL,
    service TEXT NOT NULL,
    destination_kind TEXT NOT NULL DEFAULT 'destination' CHECK (destination_kind = 'destination'),
    destination_id TEXT,
    claim TEXT NOT NULL,
    date INTEGER,
    source TEXT NOT NULL,
    source_record TEXT NOT NULL,
    mapping TEXT NOT NULL,
    native_end INTEGER,
    fields TEXT NOT NULL,
    coverage TEXT NOT NULL,
    has_location INTEGER NOT NULL CHECK (has_location IN (0,1)),
    FOREIGN KEY (payload_id, account, kind) REFERENCES service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, qso_kind, qso_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;
