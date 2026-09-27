# Durable source-bound learned sidecar review

This connects the original learned sidecar to the original FileOversight journal
and two-key publication boundary (FA-113; plan 9.2, 10.3, 10.15 and 16.4).
It does not replace inference, the planner, the congress or the rights ledger.

## Required configuration and initial packet

`FileLearnedConfig::with_required_sidecar` consumes a trusted numeric or text
recipe and binds the mandatory original sidecar gate into its exact bytes.
Existing recipe bytes remain unchanged unless this option is selected. The new
wrapper is disjoint from both legacy numeric and native-text configurations.
Every existing learned recovery entry point must match the independently held
recipe before replay. A journal requiring sidecars cannot be opened with the
corresponding optional recipe. There is no live disable or late-enable method.

The existing guarded bootstrap can publish the requirement and learned recipe
in the first canonical image. After original generation and proposal,
`FileOversight::begin_learned_sidecar` records only bounded disclosure choices.
Replay calls the original broker's source-bound constructor on the exact learned
evidence already retained for that proposal; no caller-supplied source, second
compression or recapture is used. The original input recorder installs the
packet, cumulative input charge and mandatory revision marker. Only the existing
canonical replacement acknowledges the operation. A failed write returns no
packet and leaves the owner unavailable pending exclusive recovery.

`retained_learned_sidecar` returns acknowledged historical data, not permission
or a claim of freshness. Missing clocks, paused recovery and pending numerical
intents block new preparation. Source advancement and input withdrawal invalidate
permitting reviews through the original gates. A manual text-only or even
byte-identical packet cannot install the original provenance marker. Each attempt
gets only one original planner; withdrawal cannot refill its budget.

The existing durable commit/reveal, review application, human-key and final
publication APIs consume the packet without special permission shortcuts. Saved
packets survive as historical data, but recovery withdraws old approvals and
cancels undispatched attempts through the original fence. A fresh attempt still
needs a fresh original sidecar. Reconciliation of dispatched/unknown liabilities
remains independent of new review availability.

Learned-event subtag 4 records the new sidecar commands, with preparation at its
subtag 0. All previous learned and outer journal encodings remain unchanged.
The independently retained bootstrap, cooperative writer exclusion and original
source provenance remain assumptions. This is not cryptographic authentication,
remote effect recovery or a claim that the numerical probes detect harm.
Latest-position K/V remains latest-position evidence, not a full-prefix trace.

## Outcome retrieval after completion and recovery

The existing live finish and computed-worker implementations landed concurrently.
This addition REUSES those original implementations, including their comparison
witnesses and wire format, instead of introducing another refinement reducer.

`learned_sidecar_outcome(round)` returns an acknowledged original completion by
round identity. Supervisors can recover a lost result after a response failure
or restart without repeating the round, purchasing another residual, or trying
to reconstruct a process-local handle. Records retain the original attempt,
actor revision, reviewed input revision and exact finish result. Refinements,
missing workers, exhausted budgets and original application refusals stay distinct.
The source/input revisions name the historical reviewed cut, not current eligibility.

The original live preparation records its completed result only inside the
private candidate; the sole existing persistence boundary decides when it is
acknowledged. Replay populates the same projection ONLY after the original
completion witness matches. Nothing is imported as a successful result. Original
unique-round and cumulative input limits bound retained records. A failed write
returns no candidate record and a faulted owner refuses its older RAM projection.

`read_learned_sidecar_outcome` verifies one canonical image without taking the
writer lock or creating an owner. It requires the independently supplied exact
recipe before complete original numerical/control replay. Later events must
validate too: an intact requested prefix cannot conceal a corrupt suffix. The
read cleans no staging files, appends no fence, changes no anchor or clock and
returns no role, key or live source. It is a historical cut that can race a
cooperating publication, not a promise of the newest concurrently written image.

Outcome retrieval does not report endpoint execution: an applied Allow can coexist
with a dispatched/unknown effect whose units remain charged. Likewise a missing
completion record is not a nonexecution proof. Old handles and approvals remain
withdrawn after recovery. The results contain private helper observations and are
operator-facing evidence, not an actor-facing information channel or authenticated
receipt. No existing finish algorithm, witness encoding or journal tag changes.

## Verification status

The first eight integration tests and one compile-fail boundary cover original
packet publication, forged-equivalent/manual inputs, stale source, requirement-
preserving recovery, old keys, withdrawal, budget refusal, pending intents and
legacy behavior. Seven additional integration tests and one compile-fail boundary
cover exact refined/applied outcome reconstruction, read-only access alongside a
live writer, stale live handles versus retained results, missing/budget outcomes,
original policy-application refusal, failed journal completion and corruption in
a later witness. There are fifteen integration tests and two compile-fail
examples authored by this series. Concurrent live-worker tests remain separate.
The first eight test bodies are unchanged. Model coefficients and helper ballots
are synthetic controls, not trained-detector or independent-helper evidence.

The targeted durable_learned_sidecar test and full xtask gate were attempted
through RCH_REQUIRE_REMOTE=1 rch exec. Both stopped before compilation because
rch is unavailable (exit 127). Cargo, rustc and rustfmt are also absent. Rust
compilation, tests, rustfmt and Clippy are UNEXECUTED. This preparation contains
selected source, not a complete checkout. No Bead or qualification gate is closed.
