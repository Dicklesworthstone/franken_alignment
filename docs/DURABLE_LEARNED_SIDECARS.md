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

## Compatibility and verification

The existing snapshot constructor `begin_learned_sidecar` and historical getter
`retained_learned_sidecar` keep their public behavior and request-only record.
They do not return or manufacture a current plan handle. See the
[original durable sidecar contract](DURABLE_LEARNED_SIDECAR.md).

Learned event subtag 4 contains a bounded sidecar family; existing subtag 0
retains its request-only creation format. New subtag 1 records witnessed plan
creation without reinterpreting old journal bytes. Existing learned enable/step/resume/intent
subtags 0–3 keep their encodings and meaning. Older readers reject an unfamiliar
sidecar event instead of silently dropping its provenance requirement.

Regression sources pair actual original learned generation, coarse payload
replay and two-key publication with independent recipe mismatch, copied input,
stale/foreign handles, altered comparison bytes and all five original storage
barriers. These use synthetic model and probe parameters, not trained-detector
evidence. No empirical or production safety claim follows.

Compilation, Rust tests, rustfmt and Clippy are UNEXECUTED in this environment.
The required `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check`
stops before compilation because `rch` is unavailable. Source review and exact
GitHub file verification do not substitute for execution. No Bead, roadmap
packet, restart qualification or release gate is closed by this addition.
