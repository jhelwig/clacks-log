CREATE TABLE coverage (
    coverage_id INTEGER PRIMARY KEY,
    body TEXT NOT NULL,
    account TEXT NOT NULL REFERENCES store_account(account),
    UNIQUE (coverage_id, account)
) STRICT;
CREATE TABLE lineage (
    lineage_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL REFERENCES store_account(account),
    initial_timeline TEXT NOT NULL,
    UNIQUE (lineage_id, account)
) STRICT;
CREATE TABLE timeline_boundary (
    lineage_id INTEGER NOT NULL,
    account TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    timeline_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    kind TEXT NOT NULL,
    coverage_id INTEGER NOT NULL,
    PRIMARY KEY (lineage_id, ordinal),
    UNIQUE (lineage_id, timeline_id, revision),
    FOREIGN KEY (lineage_id, account) REFERENCES lineage(lineage_id, account) ON DELETE CASCADE,
    FOREIGN KEY (coverage_id, account) REFERENCES coverage(coverage_id, account)
) STRICT;
CREATE TABLE baseline (
    baseline_row INTEGER PRIMARY KEY,
    account TEXT NOT NULL REFERENCES store_account(account),
    baseline_id TEXT NOT NULL,
    coverage_id INTEGER NOT NULL,
    lineage_id INTEGER NOT NULL,
    membership_generation INTEGER NOT NULL,
    UNIQUE (account, baseline_id),
    UNIQUE (baseline_row, account),
    FOREIGN KEY (coverage_id, account) REFERENCES coverage(coverage_id, account),
    FOREIGN KEY (lineage_id, account) REFERENCES lineage(lineage_id, account)
) STRICT;
CREATE TABLE checkpoint (
    checkpoint_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL REFERENCES store_account(account),
    baseline_id TEXT NOT NULL,
    coverage_id INTEGER NOT NULL,
    UNIQUE (checkpoint_id, account),
    FOREIGN KEY (coverage_id, account) REFERENCES coverage(coverage_id, account)
) STRICT;
CREATE TABLE installed_checkpoint (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    account TEXT NOT NULL UNIQUE REFERENCES store_account(account),
    checkpoint_id INTEGER NOT NULL,
    FOREIGN KEY (checkpoint_id, account) REFERENCES checkpoint(checkpoint_id, account)
) STRICT;
CREATE TABLE current_lineage (
    account TEXT PRIMARY KEY REFERENCES store_account(account),
    lineage_id INTEGER NOT NULL,
    FOREIGN KEY (lineage_id, account) REFERENCES lineage(lineage_id, account)
) STRICT;
CREATE TABLE accepted_record (
    envelope_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    payload_id INTEGER NOT NULL,
    stamp_id INTEGER NOT NULL,
    UNIQUE (envelope_id, account, kind, record_id),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id),
    FOREIGN KEY (stamp_id, account, kind, record_id) REFERENCES version_stamp(stamp_id, account, kind, record_id)
) STRICT;
CREATE TABLE record_observation (
    observation_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    variant TEXT NOT NULL,
    envelope_id INTEGER,
    checkpoint_id INTEGER,
    unknown_reason TEXT,
    UNIQUE (observation_id, account, kind, record_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (envelope_id, account, kind, record_id) REFERENCES accepted_record(envelope_id, account, kind, record_id),
    FOREIGN KEY (checkpoint_id, account) REFERENCES checkpoint(checkpoint_id, account)
) STRICT;
CREATE TABLE original_basis (
    original_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    variant TEXT NOT NULL,
    envelope_id INTEGER,
    checkpoint_id INTEGER,
    payload_id INTEGER,
    vector_id INTEGER,
    stamp_id INTEGER,
    unknown_reason TEXT,
    UNIQUE (original_id, account, kind, record_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (envelope_id, account, kind, record_id) REFERENCES accepted_record(envelope_id, account, kind, record_id),
    FOREIGN KEY (checkpoint_id, account) REFERENCES checkpoint(checkpoint_id, account),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id),
    FOREIGN KEY (vector_id, account, kind, record_id) REFERENCES record_vector(vector_id, account, kind, record_id),
    FOREIGN KEY (stamp_id, account, kind, record_id) REFERENCES version_stamp(stamp_id, account, kind, record_id)
) STRICT;
CREATE TABLE version_condition (
    condition_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    variant TEXT NOT NULL,
    stamp_id INTEGER,
    checkpoint_id INTEGER,
    UNIQUE (condition_id, account, kind, record_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (stamp_id, account, kind, record_id) REFERENCES version_stamp(stamp_id, account, kind, record_id),
    FOREIGN KEY (checkpoint_id, account) REFERENCES checkpoint(checkpoint_id, account)
) STRICT;
CREATE TABLE prepared_change (
    prepared_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    payload_id INTEGER NOT NULL,
    vector_id INTEGER NOT NULL,
    actor_id TEXT NOT NULL,
    condition_id INTEGER NOT NULL,
    UNIQUE (prepared_id, account, kind, record_id),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id),
    FOREIGN KEY (vector_id, account, kind, record_id) REFERENCES record_vector(vector_id, account, kind, record_id),
    FOREIGN KEY (condition_id, account, kind, record_id) REFERENCES version_condition(condition_id, account, kind, record_id)
) STRICT;
CREATE TABLE prepared_dependency (
    prepared_id INTEGER NOT NULL REFERENCES prepared_change(prepared_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    condition_id INTEGER NOT NULL REFERENCES version_condition(condition_id),
    PRIMARY KEY (prepared_id, ordinal)
) STRICT;
CREATE TABLE basis_witness (
    witness_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    original_id INTEGER NOT NULL,
    observed_stamp INTEGER NOT NULL,
    checkpoint_id INTEGER NOT NULL,
    relation TEXT NOT NULL,
    lineage_id INTEGER NOT NULL,
    UNIQUE (witness_id, account, kind, record_id),
    FOREIGN KEY (original_id, account, kind, record_id) REFERENCES original_basis(original_id, account, kind, record_id),
    FOREIGN KEY (observed_stamp, account, kind, record_id) REFERENCES version_stamp(stamp_id, account, kind, record_id),
    FOREIGN KEY (checkpoint_id, account) REFERENCES checkpoint(checkpoint_id, account),
    FOREIGN KEY (lineage_id, account) REFERENCES lineage(lineage_id, account)
) STRICT;
CREATE TABLE witness_baseline (
    witness_id INTEGER NOT NULL REFERENCES basis_witness(witness_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    baseline_row INTEGER NOT NULL REFERENCES baseline(baseline_row),
    PRIMARY KEY (witness_id, ordinal)
) STRICT;
CREATE TABLE pending_record (
    pending_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    original_id INTEGER NOT NULL,
    working_payload INTEGER NOT NULL,
    prepared_id INTEGER,
    witness_id INTEGER,
    UNIQUE (pending_id, account, kind, record_id),
    FOREIGN KEY (original_id, account, kind, record_id) REFERENCES original_basis(original_id, account, kind, record_id),
    FOREIGN KEY (working_payload, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id),
    FOREIGN KEY (prepared_id, account, kind, record_id) REFERENCES prepared_change(prepared_id, account, kind, record_id),
    FOREIGN KEY (witness_id, account, kind, record_id) REFERENCES basis_witness(witness_id, account, kind, record_id)
) STRICT;
CREATE TABLE client_record (
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    latest_observation INTEGER NOT NULL,
    pending_id INTEGER,
    PRIMARY KEY (account, kind, record_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (latest_observation, account, kind, record_id) REFERENCES record_observation(observation_id, account, kind, record_id),
    FOREIGN KEY (pending_id, account, kind, record_id) REFERENCES pending_record(pending_id, account, kind, record_id)
) STRICT;
CREATE TABLE retained_accepted (
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    envelope_id INTEGER NOT NULL,
    PRIMARY KEY (account, kind, record_id, ordinal),
    FOREIGN KEY (account, kind, record_id) REFERENCES client_record(account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (envelope_id, account, kind, record_id) REFERENCES accepted_record(envelope_id, account, kind, record_id)
) STRICT;
CREATE TABLE local_counter (
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    high_water INTEGER NOT NULL,
    PRIMARY KEY (account, kind, record_id, actor_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;


CREATE TABLE staged_accepted (
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    envelope_id INTEGER NOT NULL,
    PRIMARY KEY(account, envelope_id),
    FOREIGN KEY(envelope_id, account, kind, record_id) REFERENCES accepted_record(envelope_id, account, kind, record_id)
) STRICT;
CREATE TABLE coupled_unit (
    account TEXT NOT NULL REFERENCES store_account(account),
    operation_id TEXT NOT NULL,
    PRIMARY KEY(account, operation_id)
) STRICT;
CREATE TABLE coupled_member (
    account TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    stamp_id INTEGER NOT NULL,
    deleted INTEGER NOT NULL,
    PRIMARY KEY(account, operation_id, ordinal),
    FOREIGN KEY(account, operation_id) REFERENCES coupled_unit(account, operation_id),
    FOREIGN KEY(stamp_id, account, kind, record_id) REFERENCES version_stamp(stamp_id, account, kind, record_id)
) STRICT;
CREATE INDEX coupled_member_identity ON coupled_member(account, kind, record_id);
