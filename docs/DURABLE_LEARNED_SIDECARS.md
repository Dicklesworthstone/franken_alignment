# Durable learned sidecar congress

## Original evidence in the journaled publication path

`FileOversight::begin_learned_sidecar_plan` connects the existing journal owner to
`OversightBroker::begin_learned_sidecar`. The proposal's original learned audit
supplies the checked K/V source. The original `SidecarCongressPlan` constructs
the coarse committee input and registers its provenance at the same broker
gate used by review, approval, dispatch and checked publication. This serves
FA-113 and the plan's source-bound congress and replay requirements.

The caller supplies the existing `LearnedSidecarRequest`: disclosure identity,
ordered retained-residual priority and finite round/residual/committee-byte
ceilings. It supplies neither a numerical source nor precomputed input bytes.
No inference, compression algorithm, probe decision or authority reducer is
replaced by this adapter.

Use `FileLearnedConfig::with_required_sidecar()` before installing the generator
to require this provenance for every proposal. The exact independent recipe
binds the choice in a distinct `FALSIDE/1` wrapper. Legacy numeric, text and
stream recipe encodings remain unchanged. The wrapper can contain an existing
text or text-stream configuration; original installation and output restrictions
still apply. There is no late switch that promotes an already running source
into the mandatory mode, and no disable operation.

Mandatory sidecar mode means original **source provenance**. Trusted supervisors
still have the existing manual congress APIs. A sidecar packet is evidence,
not a numerical helper result, independent safety qualification or permit.

## Acknowledgment and recovery

The plan constructor privately replays the existing history, invokes the original
planner, and writes the request plus the exact generated sidecar payload as
comparison material. Only acknowledged canonical replacement returns a
`FileLearnedSidecar`. A storage failure returns no candidate handle or input and
retains the journal owner's original fault behavior, including an ambiguous
replacement that may already be visible.

Replay reconstructs every original learned step, the proposal's checked source,
the original disclosure plan and provenance marker. It compares the generated
payload to the retained expectation; it does not install a saved source or
trust a packet as an audit. The independent recipe must match before replay.
The payload is bounded by the existing witness ceiling and total journal limit;
priority and disclosure budgets retain their original native ceilings.

The durable handle contains only the outer owner's issuer, attempt, actor
revision and input revision. Each transaction reconstructs a fresh broker, so
the original sidecar stays private in that transaction's machine. Currentness
resolves the retained original object through the current broker instead of
reusing an obsolete broker issuer.

`current_learned_sidecar(&handle)` returns the exact committee input only while
the original source and revisions remain eligible. A pending learned step,
paused recovery, source interruption, changed actor, withdrawn/replaced input,
or different recovered owner prevents using that handle as current evidence.
There is no handle clone, mutable source or replacement planner API.

Recording caller input cannot create the original provenance marker, even when
its bytes match an independently generated sidecar packet. Replacing an existing
packet cannot reset its planner or refund its cumulative disclosure spending.
Original automatic and human keys remain separate from the input handle.

Recovery uses the existing independently configured learned open: exact original
replay followed by the same durable fence, paused numerics and fresh clock/resume
requirements. Old helper sessions and publication keys are not returned. A
read-only publication projection exposes historical endpoint state only.

## Computed votes and durable refinement

`helpers::learned::FileLearnedProbeReview` consumes the opaque current plan.
`begin_learned_probe_review` freezes the existing `LearnedWorkerSchedule`, every
member's `ProbeReviewMember` coefficients and salts, and `ProbeReviewLimits`.
The complete original committee roster must match. All scheduled evaluations
fit the admitted aggregate count; every actual evaluation additionally passes
the original per-evaluation admission, including disclosed residual costs.

The adapter privately owns original `HelperPort` objects from the existing
`Coordinator`. Each `SidecarProbeEvaluator` binds to that port, the exact current
packet and the privately retained original checked K/V. No caller-supplied source,
verdict callback, mutable evaluator, replacement member or port escapes. This is
local computation; it does not establish independent processes or statistical
independence between helpers.

`advance(&mut host, expected_revision, now, snapshot)` performs at most one input
admission, original probe, commitment or reveal for each member. Complete-roster
judgment yields before commitment. The same durable session adapter used by
socket workers records original commitments, reveal opening and reveals. Its
successful transitions require canonical acknowledgment before the coordinator
advances phases. The source is checked through the current reconstructed machine
before every numerical quantum.

All round identities are leased before the first review is returned. Existing
manual protocol APIs refuse these identities even after cancellation or driver
loss. This guarantee applies to this driver's leased rounds; it does not remove
the supervisor's other original review APIs. Dropping a driver supplies no vote
and does not refund an action or an unknown dispatched effect.

The new durable sidecar finish operation consumes the original completed session
and retains its full decision archive. When another scheduled round is available,
the original planner examines the **unapplied** review. Only its original
refinement outcome can purchase a residual and record a new source-bound input.
The previous round supplies no approval for those changed bytes. A new original
congress round must judge them, using the same frozen member definitions.

Missing members never become abstentions or a request for extra disclosure.
Exhausted residual/committee budgets cannot provide a richer packet. At the last
scheduled round, the driver applies the completed original review without buying
an unused residual. Non-refining outcomes use the original configured congress
reducer; the adapter neither replaces its quorum policy nor invents a permissive
verdict. An application refusal is an acknowledged inner error, not a retry of
the consumed round. Both automatic authorization and the separate human key
remain necessary for publication.

The finish event includes exact original result/archive comparison material.
Replay rebuilds native commitments, reveals and the original review, runs the
same refinement/application, and rejects divergence. The saved archive cannot
stand in for a live `ObservedReview`. Updated plan handles, richer inputs and
finish results escape only after canonical acknowledgment. An ambiguous failed
write exposes no result; original recovery determines the actual old-or-new cut
and fences the old sessions and keys.

The driver retains acknowledged round history, per-member evaluation status,
completed numerical work and its first failure. Cancellation, source loss,
deadline and poll exhaustion cannot retry interrupted work or erase completed
costs. Busy status after an unwind means incomplete work, not zero work. These
inspection counters describe completed local operations; the journal preserves
protocol and disclosure history, not a global physical-computation escrow.
Repeated original journal replay still incurs its existing numerical costs.

## Compatibility and verification

The existing snapshot constructor `begin_learned_sidecar` and historical getter
`retained_learned_sidecar` keep their public behavior and request-only record.
They do not return or manufacture a current plan handle. See the
[original durable sidecar contract](DURABLE_LEARNED_SIDECAR.md).

Learned event subtag 4 contains a bounded sidecar family; existing subtag 0
retains its request-only creation format. New subtag 1 records witnessed plan
creation without reinterpreting old journal bytes. Subtag 2 records original
finish/refinement, including the explicit choice to attempt refinement.
Existing learned enable/step/resume/intent
subtags 0–3 keep their encodings and meaning. Older readers reject an unfamiliar
sidecar event instead of silently dropping its provenance requirement.

Regression sources pair actual original learned generation, coarse payload
replay and two-key publication with independent recipe mismatch, copied input,
stale/foreign handles, altered comparison bytes and all five original storage
barriers. These use synthetic model and probe parameters, not trained-detector
evidence. No empirical or production safety claim follows.

Computed-driver regressions use actual coarse uncertainty, purchased original
residuals and fresh computed votes through both keys and canonical publication.
They also cover an actual member alarm, missing deadlines without refinement,
source/input loss, stale or foreign calls, cancellation, poll limits, manual
fallback refusal and all five original commitment-write barriers. Separate
finish regressions exercise original transcript reconstruction and acknowledged
old-or-new refinement outcomes. These remain authored source scenarios.

Compilation, Rust tests, rustfmt and Clippy are UNEXECUTED in this environment.
The required `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check`
stops before compilation because `rch` is unavailable. Source review and exact
GitHub file verification do not substitute for execution. No Bead, roadmap
packet, restart qualification or release gate is closed by this addition.
