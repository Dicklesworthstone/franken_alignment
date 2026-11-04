# Learned fleet Stop, Drain and retained-visit recovery

The durable shutdown coordinator now accepts independently provisioned learned
members alongside its existing ordinary members. It reconstructs each numerical
history through the original guarded recovery and uses the existing native
Stop/Fence/StopProgress consumers. Both new terminal work and passive recovery
of retained visits are supported; a saved success never replaces fresh evidence.

This is the scoped FA-014/FA-120 reference recovery path under plan sections 8.11,
16.4 and 16.9, serving FI-A02. It does not establish simultaneous distributed
shutdown, partition leases or a globally ordered post-fence frontier.

## Independent inputs and complete membership

`FileShutdownLearnedInputs` carries the original `FileLearnedConfig`, one of the
five complete `FileShutdownLearnedRequirements` variants, the required original
`RecoveryReserve`, and an optional exact `TopologyCheckpoint`. The variants are
Plain, Raw, Owned, Mediated and PredictiveMediated. Together they cover plain,
raw-predictive and owned-predictive histories with and without mediation.

The coordinator alone chooses the registered directory and bootstrap profile.
The caller supplies immutable independent inputs, never an already successful
observation, replacement owner, permit or generic recovery callback.

A full pass supplies exactly one `FileShutdownLearnedMember` for every fixed
domain. `inputs: None` explicitly selects the original ordinary path.
Missing, extra or duplicate members refuse admission. A failed learned choice
never retries through ordinary recovery. The existing full roster, clock-domain
bindings, visit and session allowances, revision growth and retained-head budgets
remain mandatory before callbacks or writes.

`run_registered_learned_pass` returns the original
`FileShutdownClockedRecoveryPass`. Its selection is one of:

| Selection | New work | Other members |
|---|---|---|
| Recover | Combined Stop/Drain for every member | None omitted |
| Resume | Original resume selector | Eligible retained visits are freshly resolved |
| RecoverSelected | Combined work for named members | Fresh passive resolution |
| StopAdmission / ResumeAdmission / StopSelected | Original admission-only phase | Original full/resume/selected rules |
| DrainStopped / ResumeStopped / DrainSelected | Original stopped-drain phase | Original full/resume/selected rules |

Admission-only selection never calls the clock. Selected work does not reduce
the report denominator or turn an unselected undrained observation into success.
A native per-member refusal can allow later members to proceed; failed
coordinator acknowledgment ends the pass and leaves the tail NotVisited.

## Original replay and terminal consumers

New visits persist the existing coordinator intent before opening the domain.
The original learned cursor holds its original exclusive Store lock. Before
event zero, the SAME captured canonical image must extend the newest whole
retained domain prefix and satisfy registered identity, bootstrap admission and
head-space limits. No unlocked preliminary history scan selects another image.

Every original numerical event executes once. Complete numerical/suffix replay,
effective policy, guard inventory, current credentials, independent floors,
predictor and optional topology requirements remain necessary. Each guarded
wrapper repeats its complete Ready checks before its private terminal transfer.

The original reducer's actual `StopProgressed` result is retained with its
original journal revision. This includes histories replayed with a proposal
diagnosis selection attached; the selector still invokes the reducer exactly
once. A subsequent passive observation therefore retains the genuine last sweep,
rather than inventing an empty drain or dropping earlier effect outcomes.

The per-domain entry points are `recover_learned_and_drain_with_clock`,
`recover_learned_admission_stop` and `drain_learned_stopped_with_clock`.
They consume only the original terminal operations. A matching completed retry
needs no clock or new domain event. An admission stop does not settle a sent
effect; a failed later clock preserves that earlier acknowledged stop. Executed,
Unknown and retention-expired effects retain their original conservative charges.
No publication is resent and no numerical or source allowance is replenished.

The original canonical checks, terminal reserve, exact event/byte capacity and
aggregate prospective-head checks still precede domain replacement. Domain and
coordinator acknowledgments remain separate; an unacknowledged coordinator write
does not prove that its domain failed to stop.

## Mediated history continuity

Mediated native inputs require an exact v2 source checkpoint. Its history may be
later than the original registration, but the complete registered prefix must
match. The same locked canonical image must extend both that checkpoint and the
coordinator's newest retained head.

The existing `FileTerminalTopologyRecovery` validates only the selected native
Stop/Fence/StopProgress successor. Other suffixes refuse. Original source
documents, work spent, operation identity and independent requirements remain
bound. The original checkpoint can therefore qualify later admission/drain or
passive recovery across the permitted terminal sequence. No v1 fallback,
refreshed source observation or source-budget reset is introduced.

## Passive retained-visit resolution

`begin_resolve_learned_shutdown_visit` returns a sealed progress/advance/finish/
cancel cursor; `resolve_learned_shutdown_visit` drives that same cursor to
completion. These methods share ordinary resolution's visit-kind, latest-cut,
capacity, pending/completed bookkeeping and coordinator-custody checks.

A learned passive read requires the exclusive domain recovery lock. It completes
the original numerical replay and full guards, then rereads canonical bytes
before returning an immutable observation. It performs no domain confirmation,
staging cleanup, clock call, recovery Fence, replacement or live-owner creation.
Ordinary resolution retains its original nonlocking behavior.

A matching pending Entered visit can acquire its original coordinator completion
without another visit slot. An already Observed visit must still match its exact
retained cut and then writes neither journal. Missing, corrupt, newer, rolled-back
or conflicting evidence withdraws current-session success; it never causes an
implicit stop retry. Explicit cancellation preserves the old durable visit.
Dropping an unfinished reader or an unwind leaves the coordinator quarantined
until reopening. Unknown endpoint outcomes stay Unknown.

## Verification checkpoint

Eight new real-history integration functions cover all six numerical/mediation
combinations, combined and separate phases, passive reopen, genuine retained
sweeps, executed/expired liabilities, eleven independent-input mutations, full
membership, cancellation, canonical races, missing/busy/corrupt domains with a
reachable control, selected recovery and passive quarantine. The complete
`mediated_learned_receipts` target now has 34 functions. Existing fixtures,
assertions and registrations remain intact; its parent adds only the new module.

The 22 changed native Rust files pass syntax parsing with the available
nightly-2026-08-31 rustfmt, and only the five new files were formatted. Independent
source reviews found no substantive blocker. These are source checks, not
compilation, borrow checking or runtime evidence.

Both prescribed remote attempts stopped before compilation with exit 127 because
`rch` is absent:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo +nightly-2026-09-08 test --locked -p fa-reference --test mediated_learned_receipts fleet_shutdown::
RCH_REQUIRE_REMOTE=1 rch exec -- cargo +nightly-2026-09-08 run --locked -p xtask -- check
```

No local compiler fallback was used. Type checking, tests, doctests, Clippy and
canonical workspace qualification remain pending; no Bead or full roadmap packet
is closed. Concurrent authenticated recovery-Stop and learned unknown-abandonment work are preserved in full.

## Runnable learned fleet operator

The Linux reference publication executable reuses the existing fleet operator:

```sh
supervise_publication shutdown-fleet create /operator/fleet.json
supervise_publication shutdown-fleet resume /operator/fleet.json
supervise_publication shutdown-fleet recover /operator/fleet.json
```

This route consistently defaults to terminal-clock execution. It supports
ordinary members and all five learned guard families in the same fixed roster:
plain, raw-predictive, owned-predictive, mediated and predictive-mediated.
Mediated members select both an original v2 topology profile and its independent
source-history anchor, covering plain/raw/owned numerical generation. The original standalone `shutdown_fleet` command
retains its ordinary behavior and rejects learned recipes before coordinator
creation.

The complete original argv is validated before flags are stripped or any
configuration is opened. Existing explicit phase, selection and coordinator
custody options are supported:

```sh
supervise_publication shutdown-fleet create /operator/fleet.json --admission-only
supervise_publication shutdown-fleet resume /operator/fleet.json --admission-only
supervise_publication shutdown-fleet recover /operator/fleet.json --drain-only --recover-domain 1
supervise_publication shutdown-fleet resume /operator/fleet.json --coordinator-anchor /custody/previous.anchor --retain-coordinator-anchor /custody/next.anchor
```

`--terminal-clocks`, `--admission-only` and `--drain-only` are mutually
exclusive. `--recover-domain` is available only with recover. Unselected members
still undergo fresh passive resolution, and the existing full denominator stays
in the report. A resume of a stopped-but-undrained member reports that fact and
does not implicitly request a new drain.

### Closed version-three profile

All existing bounds, operation identity and coordinator fields retain their
meaning. Version three adds a required explicit `plan_archive` choice and an
optional independent learned recipe per member, with paired topology custody
for mediated members:

```json
{
  "schema_version": "fa.shutdown-fleet/3",
  "operation": "900",
  "coordinator": "/operator/fleet-stop",
  "minimum_revision": "0",
  "max_visits": 128,
  "max_head_bytes": 4194304,
  "coordinator_bytes": 16777216,
  "input_bytes": 33554432,
  "runtime_ms": 30000,
  "replay_events": 8,
  "plan_archive": null,
  "members": [
    {
      "id": "1",
      "config": "/custody/learned-domain.json",
      "history_anchor": "/custody/learned-registration.anchor",
      "learned_recipe": "/custody/original-learned-recipe.json"
    },
    {
      "id": "2",
      "config": "/custody/ordinary-domain.json",
      "history_anchor": "/custody/ordinary-registration.anchor"
    }
  ]
}
```

These numbers illustrate the schema, not the appropriate allowance for an
arbitrary model or deployment. A null plan_archive requires every member's
original independent history anchor. An absolute plan archive instead selects
the existing archived-roster path, and per-member history_anchor fields refuse.
The independently retained fixed registration must stay unchanged on reopen;
the coordinator's own retained heads track subsequent observations.

A member with no learned_recipe explicitly selects ordinary recovery. A learned
member loads the exact original generation and complete independently selected guard
requirements, including current credentials and independently declared floors.
It requires the original terminal reserve. Omitting a recipe does not downgrade
an actual numerical journal into supported ordinary history.

### Mediated members retain an independent source cut

A version-three learned member may also provide both topology fields:

```json
{
  "id": "3",
  "config": "/custody/mediated-domain.json",
  "history_anchor": "/custody/original-fleet-registration.anchor",
  "learned_recipe": "/custody/original-learned-recipe.json",
  "topology_profile": "/custody/historical-topology-v2.json",
  "topology_history_anchor": "/custody/retained-source-cut.anchor"
}
```

Both topology fields require learned_recipe and must be present together.
They select the original mediated or predictive-mediated requirements for
plain, raw-predictive or owned-predictive generation. There is no new journal
format or weaker recovery route.

The topology history anchor is independent custody for the exact source
checkpoint selected by the original fa.mediated-submit/2 profile. It is not
inferred from that checkpoint. The fixed fleet registration remains unchanged;
the source cut may be later, and the native learned cursor checks the entire
registered prefix as well as the source cut. An unrelated anchor/checkpoint pair
refuses before coordinator creation.

Historical loading reuses the original topology/profile, graph-binding and
checkpoint validators. It requires v2 retained source work; v1 manual minima and
fresh checkpoint output requests refuse. The same retained pair can qualify
admission Stop, passive resume, explicit Drain and passive resume after Drain
through only the native terminal successor sequence. Previously spent checks,
reserved read bytes and source operation identity remain conserved.

Only the historical profile, initial graph/inventory, selected checkpoint and
its independent source anchor are loaded. The profile's next live graph and
inventory are not opened. No refresh, new source binding or replenished source
allowance is provisioned.

All of these successful reads share the same input_bytes allowance with
numerical recipes and assets, registration/plan custody and coordinator custody.
Every opened nested input path is checked against every registered
authority/coordinator root. An absent member store is resolved through an existing ancestor without
opening it, so native visits can report that member unavailable while reachable
members still stop.

### One bounded intake before mutation

All configuration, registration/custody and selected numerical input admission
finishes before coordinator creation or a member intent. Numerical intake reuses
the original recipe, tokenizer, model, learned codec, forecast, identity,
publication-validation and credential loaders with native review disabled.

The SAME input_bytes allowance covers the manifest, configurations, recipe files,
successful auxiliary asset reads, actual streamed checkpoint weight reads,
registration/plan archives, selected coordinator custody and historical topology
inputs. A recipe's original
asset_bytes ceiling is additionally conjoined, never enlarged. Failed streamed
weight attempts retain their actual charges. Failed input admission aborts the
command before mutable coordinator work.

Fleet numerical inputs must remain outside every registered authority and
coordinator directory. Checks cover lexical paths, resolved parent aliases and
all base/shard/adapter weight handles inside checkpoint source manifests.
A missing member store is still a legitimate fixed roster entry; resolving its
existing ancestor does not open or invent that authority. Operator-controlled
files and ancestors remain assumptions, not an adversarial-filesystem guarantee.

The loader does not read current policy evidence, invoke native/process helper
programs, open reviewer sockets, generate tokens or publish effects. Numerical
history replay is still required later; unavailable historical model/tokenizer/
codec/predictor input is not replaced by live regeneration.

One elapsed budget begins at command entry and covers intake, cooperative replay
and both sides of each required terminal clock. The old standalone ordinary
preparation retains its original checkpoint schedule. Synchronous file reads,
one numerical event, persistence and output are not hard real-time operations.

### Reports, custody and verification

The command uses the existing clocked/phased/selected JSON renderers and
per-member statuses. Partial, undrained and timed-out work stays visible and
returns an error. Coordinator custody retention precedes response output.
Write/flush failure preserves the acknowledged revision and successful archive;
it never silently retries publication or settlement.

Nine new operator test functions cover real mixed numerical histories, actual
dispatched liabilities, admission/resume/selected drain, archived busy/missing
members, exact and one-byte-short cumulative intake, nested foreign-authority
assets, complete guards and malformed suffixes, deadlines, retained custody
after write/flush loss and strict original arguments. An independent byte-size
lower bound prevents vacuous numerical-budget assertions.

Five additional operator functions cover all three mediated numerical modes
through admission/passive/drain/passive, a source cut later than registration,
unchanged actual source expenditure, independently counted five-file topology
intake, nested authority aliases, missing members with selected retry, and
closed paired v2 custody with positive controls.

All 41 owned native/operator Rust files pass syntax parsing after integration
with concurrent main work. All ten new source/test files are formatted.
Independent production and fixture reviews found no substantive blocker. The
new 22 regression functions (eight native and fourteen operator) are
**unexecuted**; syntax parsing is not type or borrow checking.

The required remote publication test invocation stops with exit 127 because
rch is absent. Automatic approval review also rejected the standalone
shutdown_fleet regression invocation because its remote destination was not
specified and could transmit repository/build inputs. That action was not run.
No local compiler fallback or remote-export workaround was used. The earlier
native targeted and canonical workspace attempts also stopped before compilation
because rch is absent. Compilation, runtime, doctests, Clippy and canonical
qualification remain open.
