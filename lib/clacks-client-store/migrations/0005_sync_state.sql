CREATE TABLE coverage (
    coverage_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL REFERENCES store_account(account),
    UNIQUE (coverage_id, account)
) STRICT;
CREATE TABLE coverage_entry (
    coverage_id INTEGER NOT NULL REFERENCES coverage(coverage_id) ON DELETE CASCADE,
    timeline_id TEXT NOT NULL CHECK (length(timeline_id) = 26),
    revision INTEGER NOT NULL CHECK (revision >= 0),
    PRIMARY KEY (coverage_id, timeline_id)
) STRICT;
CREATE TABLE lineage (
    lineage_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL REFERENCES store_account(account),
    initial_timeline TEXT NOT NULL CHECK (length(initial_timeline) = 26),
    UNIQUE (lineage_id, account)
) STRICT;
CREATE TABLE timeline_boundary (
    lineage_id INTEGER NOT NULL,
    account TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    timeline_id TEXT NOT NULL CHECK (length(timeline_id) = 26),
    revision INTEGER NOT NULL CHECK (revision >= 0),
    kind TEXT NOT NULL CHECK (kind IN ('fork','recovered')),
    coverage_id INTEGER NOT NULL,
    PRIMARY KEY (lineage_id, ordinal),
    UNIQUE (lineage_id, timeline_id, revision),
    FOREIGN KEY (lineage_id, account) REFERENCES lineage(lineage_id, account) ON DELETE CASCADE,
    FOREIGN KEY (coverage_id, account) REFERENCES coverage(coverage_id, account)
) STRICT;
CREATE TABLE baseline (
    baseline_row INTEGER PRIMARY KEY,
    account TEXT NOT NULL REFERENCES store_account(account),
    baseline_id TEXT NOT NULL CHECK (length(baseline_id) = 26),
    coverage_id INTEGER NOT NULL,
    lineage_id INTEGER NOT NULL,
    membership_generation INTEGER NOT NULL CHECK (membership_generation >= 0),
    UNIQUE (account, baseline_id),
    UNIQUE (baseline_row, account),
    FOREIGN KEY (coverage_id, account) REFERENCES coverage(coverage_id, account),
    FOREIGN KEY (lineage_id, account) REFERENCES lineage(lineage_id, account)
) STRICT;
CREATE TABLE checkpoint (
    checkpoint_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL REFERENCES store_account(account),
    baseline_id TEXT NOT NULL CHECK (length(baseline_id) = 26),
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
CREATE TABLE account_metadata (
    account TEXT PRIMARY KEY REFERENCES store_account(account),
    head_timeline TEXT NOT NULL CHECK (length(head_timeline) = 26),
    head_revision INTEGER NOT NULL CHECK (head_revision >= 0),
    baseline_id TEXT NOT NULL CHECK (length(baseline_id) = 26),
    coverage_id INTEGER NOT NULL,
    membership_generation INTEGER NOT NULL CHECK (membership_generation >= 0),
    FOREIGN KEY (coverage_id, account) REFERENCES coverage(coverage_id, account)
) STRICT;
CREATE TABLE membership_snapshot (
    account TEXT PRIMARY KEY REFERENCES account_metadata(account),
    generation INTEGER NOT NULL CHECK (generation >= 0)
) STRICT;
CREATE TABLE client_membership (
    account TEXT NOT NULL REFERENCES membership_snapshot(account) ON DELETE CASCADE,
    actor_id TEXT NOT NULL CHECK (length(actor_id) = 26),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    created_timeline TEXT NOT NULL CHECK (length(created_timeline) = 26),
    created_revision INTEGER NOT NULL CHECK (created_revision >= 0),
    closed_timeline TEXT CHECK (closed_timeline IS NULL OR length(closed_timeline) = 26),
    closed_revision INTEGER CHECK (closed_revision >= 0),
    checkpoint_id INTEGER,
    PRIMARY KEY (account, actor_id),
    UNIQUE (account, ordinal),
    FOREIGN KEY (checkpoint_id, account) REFERENCES checkpoint(checkpoint_id, account),
    CHECK ((closed_timeline IS NULL) = (closed_revision IS NULL))
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
    variant TEXT NOT NULL CHECK (variant IN ('accepted','absent','unavailable')),
    envelope_id INTEGER,
    checkpoint_id INTEGER,
    unknown_reason TEXT CHECK (unknown_reason IN ('baseline','lineage','original','collected')),
    UNIQUE (observation_id, account, kind, record_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (envelope_id, account, kind, record_id) REFERENCES accepted_record(envelope_id, account, kind, record_id),
    FOREIGN KEY (checkpoint_id, account) REFERENCES checkpoint(checkpoint_id, account),
    CHECK ((variant = 'accepted' AND envelope_id IS NOT NULL AND checkpoint_id IS NULL AND unknown_reason IS NULL)
        OR (variant = 'absent' AND envelope_id IS NULL AND checkpoint_id IS NOT NULL AND unknown_reason IS NULL)
        OR (variant = 'unavailable' AND envelope_id IS NULL AND checkpoint_id IS NULL AND unknown_reason IS NOT NULL))
) STRICT;
CREATE TABLE original_basis (
    original_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    variant TEXT NOT NULL CHECK (variant IN ('accepted','absent','subsumed','unavailable')),
    envelope_id INTEGER,
    checkpoint_id INTEGER,
    payload_id INTEGER,
    vector_id INTEGER,
    stamp_id INTEGER,
    unknown_reason TEXT CHECK (unknown_reason IN ('baseline','lineage','original','collected')),
    UNIQUE (original_id, account, kind, record_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (envelope_id, account, kind, record_id) REFERENCES accepted_record(envelope_id, account, kind, record_id),
    FOREIGN KEY (checkpoint_id, account) REFERENCES checkpoint(checkpoint_id, account),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id),
    FOREIGN KEY (vector_id, account, kind, record_id) REFERENCES record_vector(vector_id, account, kind, record_id),
    FOREIGN KEY (stamp_id, account, kind, record_id) REFERENCES version_stamp(stamp_id, account, kind, record_id),
    CHECK ((variant = 'accepted' AND envelope_id IS NOT NULL AND checkpoint_id IS NULL AND payload_id IS NULL AND vector_id IS NULL AND stamp_id IS NULL AND unknown_reason IS NULL)
        OR (variant = 'absent' AND envelope_id IS NULL AND checkpoint_id IS NOT NULL AND payload_id IS NULL AND vector_id IS NULL AND stamp_id IS NULL AND unknown_reason IS NULL)
        OR (variant = 'subsumed' AND envelope_id IS NULL AND checkpoint_id IS NULL AND payload_id IS NOT NULL AND vector_id IS NOT NULL AND stamp_id IS NOT NULL AND unknown_reason IS NULL)
        OR (variant = 'unavailable' AND envelope_id IS NULL AND checkpoint_id IS NULL AND payload_id IS NULL AND vector_id IS NULL AND stamp_id IS NULL AND unknown_reason IS NOT NULL))
) STRICT;
CREATE TABLE version_condition (
    condition_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    variant TEXT NOT NULL CHECK (variant IN ('is','absent')),
    stamp_id INTEGER,
    checkpoint_id INTEGER,
    UNIQUE (condition_id, account, kind, record_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES record_identity(account, kind, record_id),
    FOREIGN KEY (stamp_id, account, kind, record_id) REFERENCES version_stamp(stamp_id, account, kind, record_id),
    FOREIGN KEY (checkpoint_id, account) REFERENCES checkpoint(checkpoint_id, account),
    CHECK ((variant = 'is' AND stamp_id IS NOT NULL AND checkpoint_id IS NULL)
        OR (variant = 'absent' AND stamp_id IS NULL AND checkpoint_id IS NOT NULL))
) STRICT;
CREATE TABLE prepared_change (
    prepared_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    payload_id INTEGER NOT NULL,
    vector_id INTEGER NOT NULL,
    actor_id TEXT NOT NULL CHECK (length(actor_id) = 26),
    condition_id INTEGER NOT NULL,
    UNIQUE (prepared_id, account, kind, record_id),
    FOREIGN KEY (payload_id, account, kind, record_id) REFERENCES payload(payload_id, account, kind, record_id),
    FOREIGN KEY (vector_id, account, kind, record_id) REFERENCES record_vector(vector_id, account, kind, record_id),
    FOREIGN KEY (condition_id, account, kind, record_id) REFERENCES version_condition(condition_id, account, kind, record_id)
) STRICT;
CREATE TABLE prepared_dependency (
    prepared_id INTEGER NOT NULL REFERENCES prepared_change(prepared_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
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
    relation TEXT NOT NULL CHECK (relation IN ('same','before')),
    lineage_id INTEGER NOT NULL,
    UNIQUE (witness_id, account, kind, record_id),
    FOREIGN KEY (original_id, account, kind, record_id) REFERENCES original_basis(original_id, account, kind, record_id),
    FOREIGN KEY (observed_stamp, account, kind, record_id) REFERENCES version_stamp(stamp_id, account, kind, record_id),
    FOREIGN KEY (checkpoint_id, account) REFERENCES checkpoint(checkpoint_id, account),
    FOREIGN KEY (lineage_id, account) REFERENCES lineage(lineage_id, account)
) STRICT;
CREATE TABLE witness_baseline (
    witness_id INTEGER NOT NULL REFERENCES basis_witness(witness_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
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
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    envelope_id INTEGER NOT NULL,
    PRIMARY KEY (account, kind, record_id, ordinal),
    FOREIGN KEY (account, kind, record_id) REFERENCES client_record(account, kind, record_id) ON DELETE CASCADE,
    FOREIGN KEY (envelope_id, account, kind, record_id) REFERENCES accepted_record(envelope_id, account, kind, record_id)
) STRICT;
CREATE TABLE local_counter (
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    actor_id TEXT NOT NULL CHECK (length(actor_id) = 26),
    high_water INTEGER NOT NULL CHECK (high_water >= 0),
    PRIMARY KEY (account, kind, record_id, actor_id),
    FOREIGN KEY (account, kind, record_id) REFERENCES record_identity(account, kind, record_id)
) STRICT;


CREATE TABLE transfer_window (
    window_id INTEGER PRIMARY KEY,
    account TEXT NOT NULL REFERENCES store_account(account),
    source_checkpoint INTEGER NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('full','incremental')),
    after_coverage INTEGER,
    UNIQUE (window_id, account),
    FOREIGN KEY (source_checkpoint, account) REFERENCES checkpoint(checkpoint_id, account),
    FOREIGN KEY (after_coverage, account) REFERENCES coverage(coverage_id, account),
    CHECK ((mode = 'full' AND after_coverage IS NULL) OR (mode = 'incremental' AND after_coverage IS NOT NULL))
) STRICT;
CREATE TABLE window_requirement (
    window_id INTEGER NOT NULL,
    account TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    stamp_id INTEGER NOT NULL,
    deleted INTEGER NOT NULL CHECK (deleted IN (0,1)),
    PRIMARY KEY (window_id, ordinal),
    UNIQUE (window_id, account, kind, record_id),
    FOREIGN KEY (window_id, account) REFERENCES transfer_window(window_id, account) ON DELETE CASCADE,
    FOREIGN KEY (stamp_id, account, kind, record_id) REFERENCES version_stamp(stamp_id, account, kind, record_id)
) STRICT;
CREATE TABLE window_boundary (
    window_id INTEGER NOT NULL,
    account TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    timeline_id TEXT NOT NULL CHECK (length(timeline_id) = 26),
    revision INTEGER NOT NULL CHECK (revision >= 0),
    kind TEXT NOT NULL CHECK (kind IN ('fork','recovered')),
    coverage_id INTEGER NOT NULL,
    PRIMARY KEY (window_id, ordinal),
    FOREIGN KEY (window_id, account) REFERENCES transfer_window(window_id, account) ON DELETE CASCADE,
    FOREIGN KEY (coverage_id, account) REFERENCES coverage(coverage_id, account)
) STRICT;
CREATE TABLE window_baseline (
    window_id INTEGER NOT NULL REFERENCES transfer_window(window_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    baseline_row INTEGER NOT NULL REFERENCES baseline(baseline_row),
    PRIMARY KEY (window_id, ordinal)
) STRICT;
CREATE TABLE window_unit (
    window_id INTEGER NOT NULL REFERENCES transfer_window(window_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    operation_id TEXT NOT NULL CHECK (length(operation_id) = 26),
    PRIMARY KEY (window_id, operation_id),
    UNIQUE (window_id, ordinal)
) STRICT;
CREATE TABLE window_unit_member (
    window_id INTEGER NOT NULL,
    operation_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    stamp_id INTEGER NOT NULL,
    deleted INTEGER NOT NULL CHECK (deleted IN (0,1)),
    PRIMARY KEY (window_id, operation_id, ordinal),
    FOREIGN KEY (window_id, operation_id) REFERENCES window_unit(window_id, operation_id) ON DELETE CASCADE,
    FOREIGN KEY (window_id, account, kind, record_id) REFERENCES window_requirement(window_id, account, kind, record_id),
    FOREIGN KEY (stamp_id, account, kind, record_id) REFERENCES version_stamp(stamp_id, account, kind, record_id)
) STRICT;
CREATE TABLE window_progress (
    window_id INTEGER PRIMARY KEY REFERENCES transfer_window(window_id) ON DELETE CASCADE,
    generation INTEGER NOT NULL CHECK (generation >= 0)
) STRICT;
CREATE TABLE window_installed (
    window_id INTEGER NOT NULL,
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    PRIMARY KEY (window_id, account, kind, record_id),
    FOREIGN KEY (window_id, account, kind, record_id) REFERENCES window_requirement(window_id, account, kind, record_id) ON DELETE CASCADE
) STRICT;
CREATE TABLE window_staged (
    window_id INTEGER NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    account TEXT NOT NULL,
    kind TEXT NOT NULL,
    record_id TEXT NOT NULL,
    envelope_id INTEGER NOT NULL,
    PRIMARY KEY (window_id, ordinal),
    FOREIGN KEY (window_id, account) REFERENCES transfer_window(window_id, account) ON DELETE CASCADE,
    FOREIGN KEY (envelope_id, account, kind, record_id) REFERENCES accepted_record(envelope_id, account, kind, record_id)
) STRICT;
CREATE TABLE window_completed_unit (
    window_id INTEGER NOT NULL,
    operation_id TEXT NOT NULL,
    PRIMARY KEY (window_id, operation_id),
    FOREIGN KEY (window_id, operation_id) REFERENCES window_unit(window_id, operation_id) ON DELETE CASCADE
) STRICT;
