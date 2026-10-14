# Native accounting on live publication paths

Plan sections 9.9, 16–18; scoped continuation of credibility-ledger-dc0.4.
Source implementation, not execution qualification. No bead or gate is closed.

## Reserved first image and reconciled recovery

`FileOversight::create_andon_guarded_reserved` prepares the original guard set,
explicit recovery reserve, native evaluation protocol and Andon configuration
before returning an owner. The original reserve is installed immediately after
the first publication/stream bootstrap event, before guard events count as work.
Invalid reserve inputs fail before storage creation. Canonical encoding still
checks every prefix and the actual path-dependent byte limit before publication;
a later creation failure can leave an unacknowledged directory, never live roles.
The existing unreserved constructor and its bytes remain unchanged.

`FileAndonRecovery::finish_reconciled` uses the same original Fence/Time/Sweep
completion as base guarded recovery after checking the full independently held
Andon configuration, windows, protocol, history and floors. It returns the owner,
separate evaluated roles, and every original reconciliation result. The caller
must inspect outcomes AND remaining unknown/irrecoverable liabilities before
admitting new work. Returning an owner does not certify settlement.

The entire verified history is used once. The fixed original transaction owns
capacity checks, exact canonical comparison before clock and before cleanup,
replacement barriers and role publication. No new reducer, journal encoding,
effect resend or saved-score import exists. The trusted clock is called only
after verification; unwinding drops the locked cursor without issuing roles.
Ordinary Time/Sweep cannot spend a terminal-only reserve. This API bounds no
wall time and does not authenticate clocks or evaluators.

The consumer is the explicitly selected native-accounting publication workflow:
new domains need the same terminal capacity required by existing publication;
recovered domains need both native evaluation custody and settlement of prior
liabilities. Neither is replaced by a less constrained legacy opener.

## Verification

Seven authored native regressions cover reserved/legacy first-image controls,
invalid reserve admission and all original bootstrap barriers, complete original
replay parity for unexecuted/dispatched/published/expired effects, fresh evaluator
continuation, incomplete/changed canonical refusal, terminal-only capacity, and
all five completion barriers. Original base reconciliation tests and fixtures
are unchanged; the same original two-key fixture is reused for actual effects.

Required targeted tests and the unchanged xtask gate were attempted through
`RCH_REQUIRE_REMOTE=1 rch exec`. Both exited 127 before Cargo because rch is not
installed. Rust compilation, test execution, rustfmt and Clippy are unverified.
Selected-file source checks do not establish runtime or production correctness.

## Original publication commands with native accounting

The existing `supervise_publication` command now accepts an explicit ordinary
native-accounting profile before any shared option parser can remove it:

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  create CONFIG SUBMIT_JSON --andon ANDON_PROFILE \
  --retain-history-anchor /absolute/independent/first-anchor
cargo run --locked -p fa-reference --example supervise_publication -- \
  submit CONFIG SUBMIT_JSON --andon ANDON_PROFILE --guarded SUBMIT_RECOVERY_PROFILE
```

Both accept the existing optional `--reviewer-profile REVIEWER_JSON`. Omitting
that option retains the original preauthenticated/isolated reviewer-channel
assumption; it does not assert kernel peer verification. Checked, joint, learned,
service, offline-activation and other option combinations are rejected, not
silently downgraded. This increment is the ordinary one-publication workflow,
not a new daemon or signed evaluator transport.

ANDON_PROFILE is the SAME `fa.native-andon-operator/1` input and parser used by
`native_andon`; see `NATIVE_ANDON_OPERATOR.md`. Creation requires initial/current
windows to match and minimum evaluation revision zero. It publishes every guard,
the terminal reserve, evaluator protocol and native accounting in the first image.
It cannot upgrade an existing unreserved store or initialize it as a new domain.

SUBMIT_RECOVERY_PROFILE uses the original **fa.guarded-submit/1** schema, not the
receipt-only resume schema. It supplies the independently retained history anchor,
original journal/control/authority floors and mandatory `retain_history_anchor`
pointing to a NEW independent file. The Andon profile supplies the full immutable
window inventory and evaluation floor. The old anchor is never overwritten.

Recovery advances the original native cursor one event at a time, including a
Ready deadline checkpoint. Its original Fence/Time/Sweep transaction resolves
prior deliveries without resending effects. New work refuses if any original
reconciliation is unresolved or any unknown/irrecoverable liability remains.
Original actor documents retain their target, payload, epoch and deadline. The
existing `proposal-next` constructs the expected next-epoch proposal without
reserving it; stale supplied bytes still fail the actual native admission.

After creation or recovery, the owner is moved into the existing
`from_recovered` continuation. This is the actual fresh-source -> helper process
commit/reveal -> native review -> independent human approval -> publication path.
The native ledger automatically retains each successfully applied review as an
evaluation case. A helper hold does not automatically become a false stop. The
existing `native_andon assess` command subsequently applies an independent label
to that original case through the same durable source journal. Neither actors
nor helper workers receive the separate evaluator role from this command.

Known actor keys remain exact query-only retries: no source read, helper launch,
new case or new human approval. New keys use original source and publication
checks. Helper and human refusals stay original actor outcomes; stdout receives
the original wire response, not a constructed success. Retention occurs before
original child cleanup and output. A failure after acknowledged creation/recovery
still attempts to retain that actual healthy cut; uncertain storage exposes no
stale successor anchor and triggers no automatic retry or refund.

One monotonic start covers configuration ingestion, original replay and live
execution. Checkpoints do not preempt synchronous reads, writes, cleanup or
individual native operations. The original effect deadline is never extended.

Seven further authored regressions plus a synthetic helper subprocess fixture
exercise real helper processes and reviewer sockets: actual hold -> recovered
independent benign label -> charge; approval/rejection controls; new-work recovery
with retained cases; query-only retry with source removed; stale-epoch refusal;
expired/wrong-inventory admission; and closed root routing/output failure. The
legacy workflow and its existing test assertions are unchanged. These tests and
the unchanged xtask gate also stopped before Cargo (RCH absent, exit 127).
Compilation, runtime tests, rustfmt and Clippy remain unexecuted.
