# Cooperative placement of the original registered enforcers

## Capability and actual consumer

`AuthorityGraph::begin_minimum_cut` returns a sealed `CutPlanningCursor` for the
same complete authority projection, registered enforcer prices and immutable
lifetime limits as `plan_minimum_cut`. This serves the existing mediation
planner and its `FileMediationObserver` consumer (FA-072/FA-073; plan 15.6-15.9,
18; founding FI-A02/FI-A06/FI-A17). It adds no authority, effect type, journal
format, dependency, executor or deployment profile.

The synchronous sparse solver is unchanged and remains an independent execution
oracle. The cursor uses its original projection and residual representation,
including every actor root, protected sink and labeled parallel channel. It
resumes breadth-first searches at the next edge and preserves predecessor paths
through bottleneck selection and forward/reverse augmentation. Polling does not
restart previously completed work. Prices, provenance, graph identity and limits
cannot change within a task.

## Work, termination and verification

Each `advance(max_edge_visits)` spends at most its edge allowance AND the
remaining original lifetime allowance. The total work fields have the same
meaning and value as the synchronous solver. A forward/reverse residual update
reserves two visits together; `minimum_edge_visits` reports this next atomic
requirement. Repeated one-visit polls cannot half-apply that update. Zero performs
no work; callers must eventually provide the advertised minimum to make progress.

Exhaustion returns a sticky error with retained work available from `progress`.
An unwind marks the task Interrupted before it could resume a partially changed
network. No partial cut or incumbent escapes either failure. `Ready` retains a
complete plain proposal until consuming `finish`; it does not certify coverage.
The original independent `verify_cut` must still check the resulting proposal.
Bypass paths and unreachable sinks remain different, nonpermitting outcomes.

The initial sparse projection, residual allocation, queue and phase bookkeeping,
path/result materialization and one pair update are synchronous bounded work.
Counts are logical graph operations, not CPU time, heap measurements, physical
I/O or strict cancellation latency. No speedup, containment or authenticity
claim follows. A declared complete graph can still omit a real-world effect path.

## Source verification status

Eight new public regression functions compare exact outcomes and work against
the unchanged original solver, including all 1,023 nonempty forward-edge subsets
of five nodes under three price vectors and three polling quanta. Independent
exhaustive gate subsets check optimal costs, and the native independent checker
checks candidate coverage. Other controls cover all roots/sinks, parallel/reverse/
self-loop channels, pair-update boundaries, every insufficient lifetime allowance,
frozen prices/provenance, cancellation, invalid inputs and non-vacuity. Two
compile-fail examples prohibit cloning the task and elevating its output to a
VerifiedCut. Existing assertions and independent algorithms are not weakened.

These tests and Rust compilation are UNEXECUTED in the preparation environment:
`rch`, `cargo`, `rustc` and `rustfmt` are absent. The required remote-only test
launch stopped before compilation. Selected-source blob and whitespace checks
are not runtime, format, Clippy, performance or deployment qualification. No
bead is closed and prior execution receipts do not qualify this code.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test mediation_cooperative_planning
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

## Original durable owner integration

`FileMediationObserver::begin_plan_and_certify` freezes the CURRENT graph, prices,
authority epoch, control sequence and verification allowance in a
`FileMediationPlanning`. The task holds no borrowed owner, file descriptor,
writer lock or observer role. The live supervisor can record time, revoke/update
topology, fence or stop the original owner between polls. An explicitly current
journal predecessor admits time-only growth without discarding solver progress.
A changed authority epoch, control sequence, graph or availability invalidates
the task before further solver work. A caller typo or foreign-owner poll spends
no work and cannot replace the bound issuer. A recorded basis failure is sticky.

Only consuming `certify` with the ORIGINAL separately held observer and current
owner can pass the complete candidate to the original durable independent cut
checker. All bindings are checked again there, even after Ready. The fixed
verification budget cannot be enlarged at completion. Original checker refusal,
bypass and disconnected-sink results remain nonpermitting committed outcomes;
no solver-produced VerifiedCut or asserted success is imported. A failed
storage acknowledgment returns no receipt. The original canonical replacement
and poisoned-owner/recovery semantics are unchanged.

The existing synchronous `plan_and_certify` now drives this SAME owner-bound
path. The standalone synchronous graph solver is retained unchanged as the
independent differential oracle. Existing `update_and_plan` remains ordered as
before and calls the updated consumer; its original public types and body are
unchanged. A successful immutable plan is not a cached dispatch authorization.

`update_and_begin_plan` first acknowledges the ORIGINAL topology update, then
returns its original receipt and a separate fallible planning task. Missing
prices, solver exhaustion, cancellation or never polling that task cannot roll
back the update, revive an older cut, or restore withdrawn keys. Exact update
retries reuse the original update receipt without repeating its fence. A retry
for an older graph cannot plan a newer graph under the old operation. Beginning
another explicit task is a separate costed attempt, not a refill of an old task.
Neither interface refunds an unknown effect or changes existing endpoint outcomes.

Nine additional native regression functions exercise the actual original locked
file owner, full-input congress, human key, dispatch, checked publication and
reconciliation. They cover positive publication under three quanta, interleaved
time, cancellation through every small-fixture polling cut, wrong owner/observer/
predecessor calls, original Stop while planning with a dispatched liability,
late topology changes and fresh permitted work, update-before-failed-planning,
exact update retries, solver/verifier exhaustion, all five Store barriers, and
owner-drop/reopen without accidental lock or observer retention. Manual ballots
are explicit controls, not independent helper inference or accuracy evidence.
Existing test bodies and native authority/verification reducers are unchanged.

These nine tests, the eight graph tests and all three compile-fail examples are
AUTHORED, NOT EXECUTED. Remote-only RCH attempts for the owner tests and the full
integrated gate stop before compilation because the launcher is absent. No
compile, formatting, Clippy, runtime, latency or production pass is claimed.
Owner binding/snapshot checks, projection setup, the independent final verifier,
history reconstruction, encoding and storage synchronization remain synchronous;
only the planner's searches and augmentations yield at edge quanta. This does
not claim to make every publication or supervisory operation nonblocking.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference action::consequence::delivery::persistent::observed::mediation::planning
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
