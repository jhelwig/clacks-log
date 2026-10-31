CREATE TABLE clacks.record_identity (
    account TEXT NOT NULL REFERENCES clacks.accounts(account_id),
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    PRIMARY KEY (account, kind, record_id)
);

CREATE TABLE clacks.payload (
    payload_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    deleted BOOLEAN NOT NULL,
    UNIQUE (payload_id, account, kind, record_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.station_snapshot (
    station_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account TEXT NOT NULL REFERENCES clacks.accounts(account_id),
    callsign TEXT NOT NULL,
    UNIQUE (station_id, account)
);

CREATE TABLE clacks.recorded_location (
    station_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
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
    FOREIGN KEY (station_id, account) REFERENCES clacks.station_snapshot(station_id, account) ON DELETE CASCADE
);

CREATE TABLE clacks.station_reference (
    reference_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    station_id BIGINT NOT NULL,
    account TEXT NOT NULL,
    group_kind TEXT NOT NULL DEFAULT 'group',
    group_id TEXT NOT NULL,
    identifier TEXT,
    FOREIGN KEY (station_id, account) REFERENCES clacks.station_snapshot(station_id, account) ON DELETE CASCADE,
    FOREIGN KEY (account, group_kind, group_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.equipment_snapshot (
    equipment_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account TEXT NOT NULL REFERENCES clacks.accounts(account_id),
    equipment_description TEXT,
    antenna_description TEXT,
    notes TEXT,
    UNIQUE (equipment_id, account)
);

CREATE TABLE clacks.qso_payload (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'qso',
    record_id TEXT NOT NULL,
    start BIGINT NOT NULL,
    "end" BIGINT,
    own_station BIGINT NOT NULL,
    contacted_station BIGINT NOT NULL,
    own_operator_call TEXT,
    own_operator_name TEXT,
    contacted_operator_call TEXT,
    contacted_operator_name TEXT,
    power BIGINT,
    equipment_id BIGINT,
    frequency BIGINT,
    band TEXT,
    receive_frequency BIGINT,
    receive_band TEXT,
    mode_kind TEXT NOT NULL,
    mode_label TEXT NOT NULL,
    mode_parent TEXT,
    sent_report TEXT,
    received_report TEXT,
    comment TEXT,
    UNIQUE (payload_id, account),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES clacks.payload(payload_id, account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (own_station, account) REFERENCES clacks.station_snapshot(station_id, account),
    FOREIGN KEY (contacted_station, account) REFERENCES clacks.station_snapshot(station_id, account),
    FOREIGN KEY (equipment_id, account) REFERENCES clacks.equipment_snapshot(equipment_id, account)
);

CREATE TABLE clacks.qso_tag (
    payload_id BIGINT NOT NULL,
    account TEXT NOT NULL,
    tag_kind TEXT NOT NULL DEFAULT 'tag',
    tag_id TEXT NOT NULL,
    PRIMARY KEY (payload_id, tag_id),
    FOREIGN KEY (payload_id, account) REFERENCES clacks.qso_payload(payload_id, account) ON DELETE CASCADE,
    FOREIGN KEY (account, tag_kind, tag_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.tag_payload (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'tag',
    record_id TEXT NOT NULL,
    name TEXT NOT NULL,
    lotw_policy TEXT NOT NULL,
    qrz_policy TEXT NOT NULL,
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES clacks.payload(payload_id, account, kind, record_id) ON DELETE CASCADE
);

CREATE TABLE clacks.profile_payload (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'profile',
    record_id TEXT NOT NULL,
    name TEXT NOT NULL,
    station_id BIGINT NOT NULL,
    equipment_kind TEXT NOT NULL DEFAULT 'equipment',
    equipment_id TEXT,
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES clacks.payload(payload_id, account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (station_id, account) REFERENCES clacks.station_snapshot(station_id, account),
    FOREIGN KEY (account, equipment_kind, equipment_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.equipment_payload (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'equipment',
    record_id TEXT NOT NULL,
    name TEXT NOT NULL,
    snapshot_id BIGINT NOT NULL,
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES clacks.payload(payload_id, account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (snapshot_id, account) REFERENCES clacks.equipment_snapshot(equipment_id, account)
);

CREATE TABLE clacks.output_profile_payload (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'output_profile',
    record_id TEXT NOT NULL,
    name TEXT NOT NULL,
    purpose TEXT NOT NULL,
    contextual_fields TEXT NOT NULL,
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES clacks.payload(payload_id, account, kind, record_id) ON DELETE CASCADE
);

CREATE TABLE clacks.group_payload (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'group',
    record_id TEXT NOT NULL,
    name TEXT NOT NULL,
    identifier_config TEXT NOT NULL,
    bears_location BOOLEAN NOT NULL,
    catalog_binding TEXT,
    ordinary_bindings TEXT NOT NULL,
    UNIQUE (payload_id, account),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES clacks.payload(payload_id, account, kind, record_id) ON DELETE CASCADE
);

CREATE TABLE clacks.export_plan (
    payload_id BIGINT NOT NULL,
    account TEXT NOT NULL,
    profile_kind TEXT NOT NULL DEFAULT 'output_profile',
    profile_id TEXT NOT NULL,
    body TEXT NOT NULL,
    PRIMARY KEY (payload_id, profile_id),
    FOREIGN KEY (payload_id, account) REFERENCES clacks.group_payload(payload_id, account) ON DELETE CASCADE,
    FOREIGN KEY (account, profile_kind, profile_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.record_vector (
    vector_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    body TEXT NOT NULL,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    UNIQUE (vector_id, account, kind, record_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.version_stamp (
    stamp_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    timeline_id TEXT NOT NULL,
    revision BIGINT NOT NULL,
    baseline_id TEXT NOT NULL,
    vector_id BIGINT NOT NULL,
    UNIQUE (stamp_id, account, kind, record_id),
    FOREIGN KEY (vector_id, account, kind, record_id) REFERENCES clacks.record_vector(vector_id, account, kind, record_id)
);

CREATE TABLE clacks.destination_descriptor (
    descriptor_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    payload_id BIGINT NOT NULL,
    account TEXT NOT NULL,
    destination_kind TEXT NOT NULL DEFAULT 'destination',
    destination_id TEXT NOT NULL,
    collection TEXT NOT NULL,
    lotw_account TEXT,
    qrz_book TEXT,
    description TEXT NOT NULL,
    UNIQUE (descriptor_id, payload_id),
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.external_key (
    external_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account TEXT NOT NULL,
    destination_kind TEXT NOT NULL DEFAULT 'destination',
    destination_id TEXT NOT NULL,
    canonical_key TEXT NOT NULL,
    UNIQUE (external_id, account),
    UNIQUE (account, destination_id, canonical_key),
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.service_support (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    operation_id TEXT,
    acquisition_stamp BIGINT,
    descriptor_id BIGINT,
    UNIQUE (payload_id, account, kind),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES clacks.payload(payload_id, account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (acquisition_stamp, account, kind, record_id) REFERENCES clacks.version_stamp(stamp_id, account, kind, record_id),
    FOREIGN KEY (descriptor_id, payload_id) REFERENCES clacks.destination_descriptor(descriptor_id, payload_id)
);

CREATE TABLE clacks.support_requirement (
    payload_id BIGINT NOT NULL REFERENCES clacks.service_support(payload_id) ON DELETE CASCADE,
    ordinal BIGINT NOT NULL,
    requirement TEXT NOT NULL,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    stamp_id BIGINT NOT NULL,
    deleted BOOLEAN NOT NULL,
    PRIMARY KEY (payload_id, ordinal),
    FOREIGN KEY (stamp_id, account, kind, record_id) REFERENCES clacks.version_stamp(stamp_id, account, kind, record_id)
);

CREATE TABLE clacks.service_destination (
    payload_id BIGINT PRIMARY KEY REFERENCES clacks.service_support(payload_id) ON DELETE CASCADE,
    descriptor_id BIGINT NOT NULL,
    FOREIGN KEY (descriptor_id, payload_id) REFERENCES clacks.destination_descriptor(descriptor_id, payload_id)
);

CREATE TABLE clacks.service_association (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'association',
    external_id BIGINT NOT NULL,
    qso_kind TEXT NOT NULL DEFAULT 'qso',
    qso_id TEXT NOT NULL,
    state TEXT NOT NULL,
    FOREIGN KEY (payload_id, account, kind) REFERENCES clacks.service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (external_id, account) REFERENCES clacks.external_key(external_id, account),
    FOREIGN KEY (account, qso_kind, qso_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.service_suppression (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'suppression',
    external_id BIGINT NOT NULL,
    qso_kind TEXT NOT NULL DEFAULT 'qso',
    qso_id TEXT NOT NULL,
    FOREIGN KEY (payload_id, account, kind) REFERENCES clacks.service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (external_id, account) REFERENCES clacks.external_key(external_id, account),
    FOREIGN KEY (account, qso_kind, qso_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.service_disposition (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'disposition',
    observation_kind TEXT NOT NULL DEFAULT 'observation',
    observation_id TEXT NOT NULL,
    decision TEXT NOT NULL,
    association_kind TEXT NOT NULL DEFAULT 'association',
    association_id TEXT,
    FOREIGN KEY (payload_id, account, kind) REFERENCES clacks.service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, observation_kind, observation_id) REFERENCES clacks.record_identity(account, kind, record_id),
    FOREIGN KEY (account, association_kind, association_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.service_query (
    query_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    payload_id BIGINT NOT NULL REFERENCES clacks.service_support(payload_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    destination_kind TEXT NOT NULL DEFAULT 'destination',
    destination_id TEXT NOT NULL,
    stream TEXT NOT NULL,
    own_call TEXT,
    since BIGINT,
    selected_fields TEXT NOT NULL,
    UNIQUE (query_id, payload_id),
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.service_download (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'download',
    destination_kind TEXT NOT NULL DEFAULT 'destination',
    destination_id TEXT NOT NULL,
    acquisition_kind TEXT NOT NULL DEFAULT 'acquisition',
    active_id TEXT,
    FOREIGN KEY (payload_id, account, kind) REFERENCES clacks.service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES clacks.record_identity(account, kind, record_id),
    FOREIGN KEY (account, acquisition_kind, active_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.stream_progress (
    payload_id BIGINT NOT NULL REFERENCES clacks.service_download(payload_id) ON DELETE CASCADE,
    ordinal BIGINT NOT NULL,
    query_id BIGINT NOT NULL,
    watermark BIGINT,
    PRIMARY KEY (payload_id, ordinal),
    FOREIGN KEY (query_id, payload_id) REFERENCES clacks.service_query(query_id, payload_id)
);

CREATE TABLE clacks.service_acquisition (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'acquisition',
    query_id BIGINT NOT NULL,
    phase TEXT NOT NULL,
    failure TEXT,
    started_seconds BIGINT NOT NULL,
    started_nanos BIGINT NOT NULL,
    finished_seconds BIGINT,
    finished_nanos BIGINT,
    complete BOOLEAN NOT NULL,
    FOREIGN KEY (payload_id, account, kind) REFERENCES clacks.service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (query_id, payload_id) REFERENCES clacks.service_query(query_id, payload_id)
);

CREATE TABLE clacks.acquisition_input (
    payload_id BIGINT NOT NULL REFERENCES clacks.service_acquisition(payload_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    observation_kind TEXT NOT NULL DEFAULT 'observation',
    observation_id TEXT NOT NULL,
    PRIMARY KEY (payload_id, observation_id),
    FOREIGN KEY (account, observation_kind, observation_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.submission_projection (
    projection_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    payload_id BIGINT NOT NULL REFERENCES clacks.service_support(payload_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    destination_kind TEXT NOT NULL DEFAULT 'destination',
    destination_id TEXT NOT NULL,
    service TEXT NOT NULL,
    mapping_release TEXT,
    mapping_commit TEXT,
    signing_call TEXT,
    signing_dxcc TEXT,
    external_id BIGINT,
    contact_fields TEXT NOT NULL,
    station_fields TEXT,
    UNIQUE (projection_id, payload_id),
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES clacks.record_identity(account, kind, record_id),
    FOREIGN KEY (external_id, account) REFERENCES clacks.external_key(external_id, account)
);

CREATE TABLE clacks.service_attempt (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'attempt',
    qso_kind TEXT NOT NULL DEFAULT 'qso',
    qso_id TEXT NOT NULL,
    projection_id BIGINT NOT NULL,
    phase TEXT NOT NULL,
    at_seconds BIGINT NOT NULL,
    at_nanos BIGINT NOT NULL,
    batch_id TEXT,
    FOREIGN KEY (payload_id, account, kind) REFERENCES clacks.service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, qso_kind, qso_id) REFERENCES clacks.record_identity(account, kind, record_id),
    FOREIGN KEY (projection_id, payload_id) REFERENCES clacks.submission_projection(projection_id, payload_id)
);

CREATE TABLE clacks.service_result (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'result',
    destination_kind TEXT NOT NULL DEFAULT 'destination',
    destination_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    batch_id TEXT,
    outcome TEXT NOT NULL,
    outcome_detail TEXT,
    phase TEXT NOT NULL,
    at_seconds BIGINT NOT NULL,
    at_nanos BIGINT NOT NULL,
    diagnostic TEXT,
    external_id BIGINT,
    attempt_kind TEXT NOT NULL DEFAULT 'attempt',
    external_attempt TEXT,
    FOREIGN KEY (payload_id, account, kind) REFERENCES clacks.service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES clacks.record_identity(account, kind, record_id),
    FOREIGN KEY (external_id, account) REFERENCES clacks.external_key(external_id, account),
    FOREIGN KEY (account, attempt_kind, external_attempt) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.result_member (
    payload_id BIGINT NOT NULL REFERENCES clacks.service_result(payload_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    attempt_kind TEXT NOT NULL DEFAULT 'attempt',
    attempt_id TEXT NOT NULL,
    PRIMARY KEY (payload_id, attempt_id),
    FOREIGN KEY (account, attempt_kind, attempt_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.contact_report (
    report_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    payload_id BIGINT NOT NULL REFERENCES clacks.service_support(payload_id) ON DELETE CASCADE,
    own_call_state BIGINT NOT NULL,
    own_call TEXT,
    worked_call_state BIGINT NOT NULL,
    worked_call TEXT,
    start_state BIGINT NOT NULL,
    start_kind TEXT,
    start BIGINT,
    band_state BIGINT NOT NULL,
    band TEXT,
    receive_band_state BIGINT NOT NULL,
    receive_band TEXT,
    frequency_state BIGINT NOT NULL,
    frequency BIGINT,
    receive_frequency_state BIGINT NOT NULL,
    receive_frequency BIGINT,
    mode_state BIGINT NOT NULL,
    mode TEXT,
    mode_parent TEXT,
    mode_group TEXT,
    confirmation_state BIGINT NOT NULL,
    confirmation TEXT,
    UNIQUE (report_id, payload_id)
);

CREATE TABLE clacks.station_report (
    report_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    payload_id BIGINT NOT NULL REFERENCES clacks.service_support(payload_id) ON DELETE CASCADE,
    name_state BIGINT NOT NULL,
    name TEXT,
    position_state BIGINT NOT NULL,
    position_kind TEXT,
    locator TEXT,
    latitude BIGINT,
    longitude BIGINT,
    grid_length BIGINT,
    dxcc_state BIGINT NOT NULL,
    dxcc TEXT,
    primary_state BIGINT NOT NULL,
    primary_subdivision TEXT,
    secondary_state BIGINT NOT NULL,
    secondary_subdivision TEXT,
    cq_state BIGINT NOT NULL,
    cq_zone BIGINT,
    itu_state BIGINT NOT NULL,
    itu_zone BIGINT,
    references_state BIGINT NOT NULL,
    UNIQUE (report_id, payload_id)
);

CREATE TABLE clacks.reported_reference (
    reference_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    report_id BIGINT NOT NULL REFERENCES clacks.station_report(report_id) ON DELETE CASCADE,
    account TEXT NOT NULL,
    group_kind TEXT NOT NULL DEFAULT 'group',
    group_id TEXT NOT NULL,
    identifier TEXT,
    FOREIGN KEY (account, group_kind, group_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.service_observation (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'observation',
    destination_kind TEXT NOT NULL DEFAULT 'destination',
    destination_id TEXT NOT NULL,
    identity TEXT NOT NULL,
    external_id BIGINT,
    incomplete_service TEXT,
    available_fields TEXT,
    contact_report BIGINT NOT NULL,
    own_report BIGINT NOT NULL,
    counterpart_report BIGINT,
    qso_order BIGINT,
    confirmation_order BIGINT,
    acquisition_kind TEXT NOT NULL DEFAULT 'acquisition',
    acquisition_id TEXT NOT NULL,
    query_id BIGINT NOT NULL,
    observed_seconds BIGINT NOT NULL,
    observed_nanos BIGINT NOT NULL,
    FOREIGN KEY (payload_id, account, kind) REFERENCES clacks.service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES clacks.record_identity(account, kind, record_id),
    FOREIGN KEY (external_id, account) REFERENCES clacks.external_key(external_id, account),
    FOREIGN KEY (contact_report, payload_id) REFERENCES clacks.contact_report(report_id, payload_id),
    FOREIGN KEY (own_report, payload_id) REFERENCES clacks.station_report(report_id, payload_id),
    FOREIGN KEY (counterpart_report, payload_id) REFERENCES clacks.station_report(report_id, payload_id),
    FOREIGN KEY (account, acquisition_kind, acquisition_id) REFERENCES clacks.record_identity(account, kind, record_id),
    FOREIGN KEY (query_id, payload_id) REFERENCES clacks.service_query(query_id, payload_id)
);

CREATE TABLE clacks.imported_location (
    payload_id BIGINT PRIMARY KEY,
    position_kind TEXT,
    locator TEXT,
    latitude BIGINT,
    longitude BIGINT,
    grid_length BIGINT,
    dxcc TEXT,
    primary_subdivision TEXT,
    secondary_subdivision TEXT,
    cq_zone BIGINT,
    itu_zone BIGINT
);

CREATE TABLE clacks.confirmation_basis (
    payload_id BIGINT PRIMARY KEY,
    uninterpreted_band TEXT,
    uninterpreted_receive_band TEXT,
    own_call TEXT,
    worked_call TEXT,
    start BIGINT,
    band TEXT,
    receive_band TEXT,
    frequency BIGINT,
    receive_frequency BIGINT,
    mode TEXT,
    mode_parent TEXT,
    mode_group TEXT
);

CREATE TABLE clacks.service_assertion (
    payload_id BIGINT PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'imported',
    qso_kind TEXT NOT NULL DEFAULT 'qso',
    qso_id TEXT NOT NULL,
    service TEXT NOT NULL,
    destination_kind TEXT NOT NULL DEFAULT 'destination',
    destination_id TEXT,
    claim TEXT NOT NULL,
    date BIGINT,
    source TEXT NOT NULL,
    source_record TEXT NOT NULL,
    mapping TEXT NOT NULL,
    native_end BIGINT,
    fields TEXT NOT NULL,
    coverage TEXT NOT NULL,
    has_location BOOLEAN NOT NULL,
    FOREIGN KEY (payload_id, account, kind) REFERENCES clacks.service_support(payload_id, account, kind) ON DELETE CASCADE,
    FOREIGN KEY (account, qso_kind, qso_id) REFERENCES clacks.record_identity(account, kind, record_id),
    FOREIGN KEY (account, destination_kind, destination_id) REFERENCES clacks.record_identity(account, kind, record_id)
);

CREATE TABLE clacks.coverage (
    coverage_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    body TEXT NOT NULL,
    account TEXT NOT NULL REFERENCES clacks.accounts(account_id),
    UNIQUE (coverage_id, account)
);

CREATE TABLE clacks.lineage (
    lineage_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account TEXT NOT NULL REFERENCES clacks.accounts(account_id),
    initial_timeline TEXT NOT NULL,
    UNIQUE (lineage_id, account)
);

CREATE TABLE clacks.timeline_boundary (
    lineage_id BIGINT NOT NULL,
    account TEXT NOT NULL,
    ordinal BIGINT NOT NULL,
    timeline_id TEXT NOT NULL,
    revision BIGINT NOT NULL,
    kind TEXT NOT NULL,
    coverage_id BIGINT NOT NULL,
    PRIMARY KEY (lineage_id, ordinal),
    UNIQUE (lineage_id, timeline_id, revision),
    FOREIGN KEY (lineage_id, account) REFERENCES clacks.lineage(lineage_id, account) ON DELETE CASCADE,
    FOREIGN KEY (coverage_id, account) REFERENCES clacks.coverage(coverage_id, account)
);

CREATE TABLE clacks.checkpoint (
    checkpoint_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account TEXT NOT NULL REFERENCES clacks.accounts(account_id),
    baseline_id TEXT NOT NULL,
    coverage_id BIGINT NOT NULL,
    UNIQUE (checkpoint_id, account),
    FOREIGN KEY (coverage_id, account) REFERENCES clacks.coverage(coverage_id, account)
);

CREATE TABLE clacks.accepted_record (
    envelope_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    payload_id BIGINT NOT NULL,
    stamp_id BIGINT NOT NULL,
    UNIQUE (envelope_id, account, kind, record_id),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES clacks.payload(payload_id, account, kind, record_id),
    FOREIGN KEY (stamp_id, account, kind, record_id) REFERENCES clacks.version_stamp(stamp_id, account, kind, record_id)
);

ALTER TABLE clacks.sync_baselines
    ADD COLUMN baseline_row BIGINT GENERATED ALWAYS AS IDENTITY UNIQUE,
    ADD COLUMN coverage_id BIGINT NOT NULL REFERENCES clacks.coverage(coverage_id),
    ADD COLUMN lineage_id BIGINT NOT NULL REFERENCES clacks.lineage(lineage_id);
ALTER TABLE clacks.sync_accounts
    ADD COLUMN installed_coverage_id BIGINT NOT NULL REFERENCES clacks.coverage(coverage_id),
    ADD COLUMN current_lineage_id BIGINT NOT NULL REFERENCES clacks.lineage(lineage_id),
    ADD COLUMN retained_journal_coverage_id BIGINT NOT NULL REFERENCES clacks.coverage(coverage_id);
ALTER TABLE clacks.desktop_clients
    ADD COLUMN acknowledged_checkpoint_id BIGINT REFERENCES clacks.checkpoint(checkpoint_id);
DROP TABLE clacks.sync_baseline_coverage;

CREATE TABLE clacks.accepted_head (
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    envelope_id BIGINT NOT NULL,
    PRIMARY KEY (account, kind, record_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES clacks.record_identity(account, kind, record_id),
    FOREIGN KEY (envelope_id, account, kind, record_id) REFERENCES clacks.accepted_record(envelope_id, account, kind, record_id)
);
CREATE TABLE clacks.installation_journal (
    account TEXT NOT NULL,
    timeline_id TEXT NOT NULL,
    revision BIGINT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    stamp_id BIGINT NOT NULL,
    PRIMARY KEY (account, timeline_id, revision, kind, record_id),
    FOREIGN KEY (stamp_id, account, kind, record_id) REFERENCES clacks.version_stamp(stamp_id, account, kind, record_id)
);
CREATE INDEX accepted_qso_membership ON clacks.qso_tag(account, tag_id, payload_id);
ALTER TABLE clacks.destination_descriptor ADD FOREIGN KEY (payload_id) REFERENCES clacks.service_support(payload_id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE clacks.imported_location ADD FOREIGN KEY (payload_id) REFERENCES clacks.service_assertion(payload_id) ON DELETE CASCADE;
ALTER TABLE clacks.confirmation_basis ADD FOREIGN KEY (payload_id) REFERENCES clacks.service_assertion(payload_id) ON DELETE CASCADE;
