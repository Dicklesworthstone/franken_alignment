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
