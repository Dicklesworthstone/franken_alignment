# Durable policy sources for original learned review

Implementation status: source integration, not execution-qualified. Plan roots:
sections 6.4–6.5 (ordered authority and separate clocks), 7.2–7.6 (observations,
closure and currentness), 8.3 (one-shot effects), and 9.3/9.11 (separate judgment
and human authority). Existing open gate/bead acceptance is not relaxed or closed.

## Bootstrap two independent requirements

Build the original text or text-stream recipe, then select both requirements:

```rust
let config = config
    .with_required_sidecar()?
    .with_required_policy_source(policy)?;
```

`policy` is the existing `FileSourcePolicy`: exact `StateSource`, `StateLimits`
and `StateFreshness`. A numerical-only recipe or a recipe without mandatory
sidecar provenance refuses this mode. The complete independent recipe binds
source scope/identity/generation, event/byte limits and freshness as well as all
original numerical, tokenizer, sampling, monitor and sidecar parameters. It is
not reconstructed from an untrusted source file or from numeric labels alone.

The original learned Enable event installs the original leased policy-state
capture and its sole writer. The `FALPOLI` configuration wrapper is disjoint from
the existing recipe wrappers; it uses the existing source-policy serializer.
There is no new source event format, authority ledger, source replay engine,
publication endpoint, permitting route or public provider extension.

`file_source_required()` remains true. `policy_only_file_source_required()` names
the additional immutable role. An existing full-context source cannot be
relabelled. Original source replacement changes its registered generation and
authority epoch under the existing law; it preserves the policy-only role and
producer/semantic/equal-version byte floors. The bootstrap getter reports the
original contract, while `file_source_status()` reports the live source state.

## Policy files do not replace learned helper input

Files must have exactly the registered scope/source, the complete helper roster,
a complete policy snapshot, and EMPTY helper contexts. The original durable
source transaction enforces this, including replay. It keeps a correctly scoped
newer producer version as a floor even when that version has incomplete policy
or an invalid roster/context. Reopening with a new reader cannot revert to an
earlier complete version, replace same-generation bytes, or roll back the
semantic epoch under a newer producer generation.

The source's policy snapshot feeds the original deterministic gate. Helper
inputs still come from the original learned sidecar planner and its registered
input-revision provenance. A caller-made file-context packet cannot create that
registration, start a permitting review, or authorize an effect. The original
full-context role retains its exact file-input equality rule unchanged.

Changing source bytes or losing source eligibility withdraws the original input
basis and human keys. Refreshing a later good file does not restore an old
sidecar judgment, reset its work budget, or manufacture either effect key.
Policy observation and effect admission are distinct facts.

## Use the existing actor and driver paths

The source-only actor wire/channel/peer/listener methods described in
`LEARNED_TEXT_WIRE.md` now select the ORIGINAL durable intake when this recipe is
installed. New valid intents reread and acknowledge the source, validate the
post-read lease, then consume the original one-use snapshot during original
request admission. Retries, malformed intents, poll/cancel and unconfirmed stream
finish retain their read-free behavior. Direct policy preparation can repair a
source during recovery; it cannot resume numerical generation or admit output.

The native driver's `step_from_policy_file` and the computed-probe driver's
`step_computed_from_policy_file`, including their credentialed variants, share
one private provider. It resolves original learned provenance before acquisition,
refreshes the existing durable source, then resolves the full original input
again. Every native/probe review quantum, both captures around automatic
reservation, and first publication use the same source gates. Query-only
reconciliation does not read the source or require a live learned plan.

An expired lease is not the same as withdrawn learned provenance. The private
acquisition preflight can reread an expired policy while original numerical
input remains current. It cannot return a permitting input until the ORIGINAL
lease and full source check pass afterward. All public permitting paths retain
the full check. A changed/withdrawn input still fails before another file read.

For an independent human request, use
`FileSupervisedDriver::request_learned_human_approval_from_policy_file` or the
native wrapper's `request_human_approval_from_policy_file`. These refresh policy,
check the post-read clock and original current input, and freeze the existing
request. They never approve it or obtain the separately provisioned reviewer.

Unconfigured legacy policy readers retain their previous reader-local version
floor and no durable lease claim. Legacy full-context sources remain incompatible
with policy-only learned evidence. There is no automatic journal migration,
bootstrap upgrade or weaker recovery fallback.

## Reports, failure and recovery

`FileEvidenceReport.source_updates` now records each original durable refresh,
including committed refusal, read error plus withdrawal failure, or journal
failure. `observations` records only actual observations. A journal failure
supplies no invented producer identity; a successful source observation does not
assert successful request admission or publication. Actor responses remain the
existing redacted response type, without source paths or private policy values.

Read-start leases charge read/parse/persistence latency; post-read clock checks
cannot relabel an old closure as a fresh one. Policy or clock failure cannot
reuse the previous one-use admission slot. Source loss after automatic
reservation does not silently refund it. After dispatch, failure may prevent
first publication, but only the original terminal receipt/reconciliation law
can resolve execution and liability. Recovery retains source floors, withdraws
live source eligibility, fences old rights and leaves generation paused.

These are reference contracts: selected files/ancestors must be outside actor
write authority. The source identity is not authentication; a complete policy
snapshot is not a complete external-world observation. Socket budgets do not
bound synchronous file, journal or historical numerical replay latency.

## Authored regression coverage and execution status

Nineteen new regression functions cover bootstrap binding, original leases,
reader/recovery rollback and equal-version substitution, refused-version floors,
semantic rollback, full-context compatibility, all five source storage barriers,
wire intake, actual native allow/deny, computed probes, renewed leases during
review and human requests, refused file-context substitution, loss after
reservation and after dispatch, query-only unknown-outcome recovery, and both
stream append and independently reviewed receipt-confirmed finish. Synthetic
weights establish intended mechanics, not detector effectiveness/independence.

Both targeted and full mandatory RCH commands were attempted and could not
launch (`rch: command not found`, exit 127). No local compiler fallback was used.
Compilation, tests, rustfmt and Clippy are unexecuted. Preimage hashes and source
review do not substitute for those gates. Raw attempts are retained in
`artifacts/execution/2026-10-05-learned-policy-source.log`.
