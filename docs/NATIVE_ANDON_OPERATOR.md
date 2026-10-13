# Native Andon operator

The `native_andon` Unix reference example connects operator files to the existing
`FileOversight` native evaluation/accounting journal. It shares the original
`supervise_publication` configuration parser and exact-history custody code.
There is no alternate evaluator, journal encoder, authority owner or source reader.
Plan sections 9.9, 16–18; FI-A08/FI-A13/FI-A18; scoped continuation of
`franken_alignment-credibility-ledger-dc0.4`. No bead or execution gate is closed.

## Guarded creation and inspection

```sh
cargo run --locked -p fa-reference --example native_andon -- \
  create CONFIG ANDON_PROFILE /absolute/independent/new-anchor
cargo run --locked -p fa-reference --example native_andon -- \
  inspect CONFIG ANDON_PROFILE RECOVERY_PROFILE
```

CONFIG is the original `fixtures/supervised_publication.json` format. It must
name the exact original publication profile and source identity. Parsing helper
program settings does not execute them or read current evidence. These commands
support the ordinary source-publication guard set: source required, with no
stream, numerical, identity, campaign, credential or joint-evaluation fallback.

ANDON_PROFILE uses `fa.native-andon-operator/1`; the complete example is
`crates/fa-reference/fixtures/native_andon_operator.json`. Its evaluation protocol
is passed to the original native validator. `accounting.initial_windows` is the
immutable bootstrap inventory; `current_windows` includes every later registered
window, including windows without charges. `minimum_evaluation_revision` is an
independently retained floor. Creation requires identical initial/current windows
and revision zero; it cannot import an old deployment's accounting as a new one.

RECOVERY_PROFILE uses the existing `fa.guarded-resume/1` custody schema:

```json
{
  "schema_version": "fa.guarded-resume/1",
  "minimum": {"journal_revision": 4, "control_sequence": 0, "authority_epoch": 0},
  "history_anchor": "/absolute/independent/acknowledged-anchor"
}
```

The numbers above illustrate fields, not safe default recovery floors. Retain
actual values and the exact anchor independently. Inspection rejects any
`retain_history_anchor` request and never creates a live owner, cleans staging,
advances time or fences. It works beside a locked writer and returns only the
validated historical cut. Pending and censored cases remain explicit. The JSON
report includes native window counts, wide costs and incident-opening rounds,
not payloads, helper questions, label evidence bytes or executable paths.

Creation uses the original atomic guarded bootstrap and requires a new anchor
output outside the authority store. Existing outputs are never overwritten;
exclusive create is checked again when writing. The original owner acknowledgment,
independent anchor retention and stdout are separate boundaries. A slow write or
lost stdout does not undo creation, repeat it, or prevent retention from being
attempted. An export failure leaves any partial output diagnostic. The caller
must retain the original store and resolve that failure, not recreate the domain.

One monotonic invocation budget begins before CONFIG ingestion. Checkpoints
surround configuration, custody, native work and read-only reporting. Existing
file reads, decoding, creation and synchronization remain synchronous, so this
is cooperative deadline checking, not preemption or a wall-time guarantee.
The creation command does not install a terminal recovery reserve; all subsequent
operations obey the original finite event/byte limits. No reserve is inferred.

## Verification status

Six authored regressions cover real guarded creation/locked inspection, strict
input boundaries, exact configuration refusal, expired invocation admission,
independent custody, and write/flush failures after acknowledged creation. They
use the original configuration fixture and native filesystem owner, not a
substitute journal. No current helper or evaluator authentication is claimed.

This environment has no RCH launcher; required example tests and the unchanged
xtask gate exit 127 before Cargo. Compilation, Rust tests, rustfmt and Clippy are
unexecuted. Source and Git-object checks do not qualify runtime semantics.

## Recovered assessments and new windows

```sh
cargo run --locked -p fa-reference --example native_andon -- \
  assess CONFIG ANDON_PROFILE RECOVERY_PROFILE ASSESSMENT_JSON
cargo run --locked -p fa-reference --example native_andon -- \
  register-window CONFIG ANDON_PROFILE RECOVERY_PROFILE WINDOW_JSON
```

These mutating invocations require `retain_history_anchor` in RECOVERY_PROFILE,
pointing to a NEW independent file. The previous archive is never overwritten.
The complete exact original protocol/window/guard inventory, floors and anchor
are admitted before replay. The operator advances the original `FileAndonRecovery`
one event at a time and checks the SAME invocation deadline at every boundary,
including Ready. Expiry before Ready completion publishes no fence or label and
leaves staging untouched. Recovery finish acknowledges one original fence before
returning fresh evaluator/human roles; the command never exports those handles.

ASSESSMENT_JSON uses `fa.native-assessment/1`, with unsigned `round`, `attempt`,
`policy_generation`, `congress_generation`, `expected_evaluation_revision`, and
`origin`; `case_evidence_root_hex` and `evidence_id_hex` are lowercase 32-byte
nonzero identities; `truth` is exactly `benign`, `violation` or `censored`.
All fields are mandatory and unknown fields refuse. The actual original applied
case must match the supplied attempt, policy/congress generations and root.
New labels require the expected native evaluation revision. Exact existing labels
still use the original ticket/evaluator check and append no assessment or Time.
The operator never manufactures a missing case or substitutes committee consensus
for independent evaluation. Label files are trusted custodian input, NOT signed
or authenticated evaluator messages; do not give this command to an actor.

Only a NEW benign assessment acquires checked Unix-millisecond time, after
verified replay and with deadline checks before and after acquisition. A failed
clock never becomes tick zero. Censored/violation labels and exact retries need
no clock. Time observation and assessment remain TWO original transactions: a
late deadline or refusal can leave an acknowledged Time without a label. The
successor anchor includes the actual acknowledged cut. Each invocation performs
one recovery fence even for a label retry; it does not pretend to be a no-write
restart. Existing source snapshots, helpers and human approval are not consulted.

WINDOW_JSON uses `fa.native-andon-window/1` and unsigned `window`, `start`, `end`,
`false_stops_allowed`, `cost_per_stop`. The original immutable/nonoverlapping law
is used. After adding a window, retain the returned anchor/floors and add that
EXACT window to ANDON_PROFILE.current_windows; a later invocation using an
incomplete inventory refuses instead of accepting discovered configuration.

Mutation output uses `fa.native-andon-mutation-result/1`: `applied`,
`already_applied` or `refused`, the acknowledged recovery-fence revision and a
nested original accounting report. Native operation acknowledgment, independent
anchor retention and output are distinct. Healthy refusal/timeout after recovery
still attempts to retain the acknowledged fence/Time cut. An uncertain storage
replacement returns an error, exports no stale successor anchor and preserves
the previous archive; reopen original history to determine what persisted. No
command automatically retries, repairs staging, resends an effect or refunds.

Ten further authored regressions exercise real applied native congress cases and
original file recovery: offline benign/censored/violation assessment, exact retry,
case/revision mismatch, unavailable/slow clock, every replay/Ready checkpoint,
uncertain Time publication with original corrected recovery, immutable window
registration, output write/flush failure, closed documents/custody and actual
assessment command routing. Sixteen regressions are now authored for this example.
Both new RCH test/gate launches also exit 127 before Cargo; compilation, Rust
runtime tests, rustfmt and Clippy remain unexecuted. Numerical/joint/other guard
compositions and cryptographically authenticated evaluator intake are not added.
