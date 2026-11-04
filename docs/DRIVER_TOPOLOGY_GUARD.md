# Required topology acquisition in the existing publication driver

## Source capability, not runtime qualification

The original `FileSupervisedDriver` can now retain one `FileDriverTopology`.
`require_topology` transfers the existing separately held `FileTopologySource`
into the idle driver after checking the exact original owner, graph and journal
predecessor. Setup performs no read or certification. A rejected setup returns
the original custody, including spent work, for a corrected retry. The source is
not replaceable or removable through a running driver's API.

This connects the live source to the existing ordinary, concrete-file, computed,
learned-evidence and credential-aware stepping methods: all their publication
paths converge on the same original `publish_job`. Selection does not require a
caller to remember a different step method on each request. Neither the actor
port, the helper input nor the publication keys receive topology-source custody.

For unresolved, unexpired first publication, the driver acquires and validates
its original policy/helper evidence, refreshes the selected topology source,
then acquires the original post-capture clock and invokes the unchanged native
publication gate. Both original approvals have already been consumed by dispatch.
A topology refresh cannot replace missing helper evidence or an absent required
credential. An acknowledged source withdrawal reaches the original sealing path;
only later native reconciliation can refund a recorded nonexecution. Source or
storage failure without acknowledgment prevents the final clock/publication call.
The existing driver retires its send path before entering fallible source work.

The original endpoint classifier, not counters or this source's availability,
decides when evidence is needed. Already resolved/expired/unsendable obligations
skip all new topology reads and operation allocation. Reconciliation, idle
polling, helper review and human approval do not spend topology read capacity.
The prior actual source report is retained, not relabeled as a fresh success.

## Custody, work and historical diagnostics

The source's original monotonically increasing operation IDs supply automatic
refresh identities, including after prior explicit source use. Identifier
overflow never wraps or retries an old withdrawal: the owner is quarantined and
the error remains explicit. Original source limits and spent check/read capacity
survive every request and driver phase. Refresh does not allocate a new allowance.

`topology()` exposes only immutable source diagnostics and the last
`DriverTopologyObservation`. That record identifies its original request,
attempt and starting journal revision. It is installed as `Interrupted` before
source entry and becomes `Finished` only when that call returns. It preserves
an acknowledged source refusal separately from an unacknowledged journal error.
It is historical supervisor data, not a current cut, endpoint outcome or key.

`refresh_topology` supplies initial certification or an explicitly scheduled
additional check with the same custodian and work. `rebind_topology` delegates
to the original explicit newer-graph/newer-inventory update; it cannot adopt
changed files automatically, change paths, enlarge limits or clear spent work.
A successful rebind still needs a fresh check before its new graph is certified.

`release()` returns the SAME selection in `FileDriverRelease::topology`, together
with the original supervisor, retained children and learned-review evidence. A
successor can use the checked `FileDriverRelease::into_driver` handoff below or
explicitly reattach that value. Failed attachment returns it unchanged. No source or observer is cloned. An actual owner restart changes
issuer identity, so old custody cannot attach to its newly recovered owner.

## Trust and scope

This is an explicit process-local supervisor composition, not a persisted
mandatory deployment policy, OS confinement, a background watcher or a CLI
rollout. A trusted supervisor still has its original direct owner interfaces and
can deliberately choose a differently configured driver. Actor isolation,
authenticated provisioning, complete modeled topology and filesystem behavior
remain the source's original assumptions. Original storage and temporal gaps
between separately acknowledged source and publication operations remain.
Reads, decoding, independent graph verification, numerical/journal reconstruction
and storage synchronization are synchronous. No latency guarantee, empirical
containment improvement, runtime speedup or learned-detection result is claimed.

The work serves FA-072/FA-073, plan 15.2/15.6 and 17.1, and FI-A02/FI-A06. It
changes no journal encoding, reducer, cut checker, executor or dependency.

## Initial regression set and pending verification

Seven new regression functions drive actual Unix socket congress, original
actor requests, both original approval keys and the journal-as-publication sink.
They cover two requests retaining source work, source loss inside the policy
callback through ordinary/computed entry points, missing whole inputs despite a
valid graph, original resolved receipt priority, both deadline boundaries,
source exhaustion on a later request, and failed source attachment with corrected
retry. Fixed manual helper responses are controls, not inference or accuracy
qualification. One compile-fail example denies cloned source selection.

The Rust tests, compilation, formatting and Clippy remain UNEXECUTED. The
prescribed RCH launcher is absent; source/hash/patch checks cannot substitute for
execution. No bead or production qualification gate is closed. The required
commands for the integration owner are:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference action::consequence::delivery::persistent::observed::driver::topology
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

## Checked same-owner handoff and further regression controls

`FileDriverRelease::into_driver` now transfers the original supervisor,
selected source, accumulated work, last source observation, terminal computed
review and child-reap duty back into an idle original driver. No caller needs
to dismantle the release just to restore a loop, accidentally dropping the
required source. Existing jobs and consumed keys are NOT reconstructed. A
publication interrupted by release still has only its original ledger and
endpoint reconciliation paths; handoff cannot retry its send.

The release's fields are public data, so this method rechecks owner health and
the exact source/graph binding. A substituted supervisor or failed owner refuses
and returns the COMPLETE release in `FileDriverHandoffError`, including every
child-reap obligation. This consumes neither an observer nor the custody needed
for a corrected same-owner handoff. Source operation IDs are inspectable through
`topology().source().last_operation()` so an explicit newer rebind can choose a
fresh ID without guessing how many automatic or unavailable checks ran.

Seven further regression functions cover release during an outstanding dispatch,
substituted-owner handoff followed by corrected transfer, loss followed by old-file
refusal and a newer rebind with fresh native review/publication, operation-ID
overflow, all five original source-persistence barriers, a post-source clock
unwind, and original credential-required publication with/without its real key.
The credential inventory and fixed helper responses are explicit controlled
inputs, not authentication or empirical learned-detection evidence. A second
compile-fail example forbids duplicating the consumed release.

In total fourteen regression functions and two compile-fail examples are
AUTHORED, NOT EXECUTED. The targeted suite and full remote-only gate were
attempted again for the final source; both stop before compilation because
`rch` is absent. Child ownership preservation is implemented by transfer of the
original value, not a claimed process/descendant-containment test result.

## Closing the explicit adapter's policy-read publication gap

`EvidenceProvider::capture_for_publication` is private to the original driver.
It is called only after the existing exact-attempt publication preflight says
fresh evidence is needed, never by review, approval, dispatch or reconciliation.
Legacy providers inherit their existing capture unchanged. The explicit topology
file decorator acquires policy first, then refreshes its original topology source
once, then returns to the original validation, required-source check and final
clock/endpoint gate. It no longer spends a pre-policy topology check at this cut.

The earlier topology-first launch, review, human request and two dispatch capture
contracts are unchanged. An outer policy/storage failure reaches neither another
source observation nor publication. An inner policy refusal still runs the final
topology observation and can reach only the native nonexecution seal. A native
source withdrawal is not treated as a persistence failure; an unacknowledged
source operation stops before final time or endpoint work. No charge is refunded
by an observation or an error. Only original reconciliation can resolve it.

The selected driver source is checked AFTER any explicit adapter's capture, so
choosing another step API does not disable the required selection. Supplying an
additional independently held source does not replace or refill the selected
source; each retains its own actual work and native failure boundaries.

This closes the explicit observation ordering gap, not an OS-level atomicity
claim: trusted paths must still be outside actor write authority. Concurrent
filesystem changes after the final observation remain outside this reference
contract. Controlled callback faults in tests are not learned detector evidence.

Seven additional publication-order regressions are authored: topology loss during
policy acquisition for either file; inner versus outer policy failure; all five
native source-persistence barriers; unchanged topology-first pre-publication
capture; resolved/expired source-free priority; and an actual bounded policy file
whose exact inputs pass the original socket review and two-key publication.
The controlled callback adapter is reachable only in tests; production still uses
sealed file readers. Combined with the fourteen retained driver tests, this is
21 authored regression functions and two compile-fail examples, not executed
validation. The required RCH checks and targeted test launch stopped before compilation
(exit 127): `rch` is unavailable in this editing environment.
