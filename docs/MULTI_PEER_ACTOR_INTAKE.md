# Bounded multi-peer actor intake

`FileActorPool` composes the existing `FileActorInbox` and `FileSupervisedDriver`
so independently connected peers can make progress while another peer has an
incomplete frame or a blocked response. It lives in
`requests::actor::source_wire::inbox::pool`. This is L5 ingress composition serving
FA-107 and plan sections 9.3, 15 and 17.1, not a new authority domain or actor verb.
It retains the founding one-directional boundary: actors submit proposals and
receive the original restricted `Knowledge` responses; they never receive source
observations, helper packets, ballots, controller handles or effect keys.

## Operation

Construct one to sixteen operator-named original inboxes. Each keeps its own fixed
kernel credential policy, ticket inventory, pending reply, connection quota and
ready queue. The constructor rejects empty/oversized sets, zero names and duplicate
names, returning every original inbox on failure. `attach(peer, socket)` selects a
registered session; the existing SO_PEERCRED check still happens before frame reads.
Only a trusted acceptor chooses this name. It is not an in-band actor identity and
must not be selected by an unauthenticated request. Reconnecting an explicit session
preserves its tickets and lifetime limits; another session cannot poll them without
the original exact-submission retry. No ticket map is merged across peers.

`drive(driver, source, clock, PoolBudget)` validates both budgets and EVERY inbox's
original gateway owner before any socket read or source operation. A foreign member
or faulted shared owner refuses the entire call, including otherwise valid peers.
The bounded visit vector is allocated before work. Round-robin transport visits
use the minimum of a per-peer quantum and a shared remaining socket budget. Each
peer is visited at most once per call. Ineligible peers cannot reset the cursor
behind the peer that used the shared allowance. Buffered frame work and pending
flushes are eligible without a new read-readiness event.

The total caps read bytes, written bytes, complete frames and I/O attempts across
ALL peers; it is not independently multiplied by the peer count. A failed original
drive has no partial counter report, so the pool charges its entire issued allowance
and stops before visiting another peer. The successful outer Result represents
completed preflight, not success of every visit: inspect `stopped_on_error()` and
each visit's exact result. The report keeps all preceding successful visits. Closed
transports disconnect without deleting accepted work or refunding effects.

`next_request(driver)` uses a separate fair cursor and the original journal-backed
queue checks. It requires the original driver to be Idle. FIFO order is retained
inside a peer; peers alternate when several have work. Cancelled and terminal hints
are discarded only after reading their actual journal state. Dispatching and Unknown
remain reconciliation-only. Revoking any or all ingress sessions does not erase
ready obligations or claim that an effect was cancelled. `into_inboxes()` returns
the same owners without resetting their session or connection limits.

## Bounds and limitations

There are at most sixteen existing inboxes, each with its original bounded queue
and frame limits. Construction does not combine principals or tenants: all members
must match the one original supervisor. Independent supervisor/authority domains
require independent pools. Socket work is bounded, but synchronous source reads,
journal replay/replacement, allocation latency and helper computation are not a
hard real-time deadline. This is cooperative scheduling, not a new executor,
Asupersync admission, an OS sandbox, a multi-process isolation proof or a guarantee
against a stalled filesystem. The existing publication command is not changed by
this library addition. Source/clock authenticity remains the original profile's
operator assumption. No grant, wire format, journal tag or dependency is added.

## Source and verification status

Nine `pool_` Rust regressions are authored over actual Unix sockets, registered
source files and original durable owners. Positive controls cover simultaneous
intake, fair queue selection, tiny versus sufficient shared budgets, reconnect and
exact retry. Negatives cover incomplete frames, blocked output, cross-session poll,
cancellation, foreign owners, zero/invalid budgets, constructor limits, revoked
peers and a real staging-file collision that must stop later peers. These are
same-process Unix sockets, not an isolated hostile-process deployment.

The required targeted attempt failed before compilation (`rch: command not found`,
exit 127):

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference pool_
```

Rust compilation, all nine tests, compile-fail checks, rustfmt, Clippy and the full
`xtask check` gate are UNEXECUTED. Static source/byte review does not qualify runtime
behavior. The Beads CLI is unavailable; no bead or execution gate is closed and no
historical qualification is claimed for this addition.


## Generated text and observation-only servicing

`FileActorPool<FileGeneratedTextActorPort>` now uses the SAME scheduler, global
socket allowance, peer quanta, independent tickets and ready queues. Its source
preparation calls the original fixed native-intent decoder before registered-file
acquisition. Only an already computed generation reference or explicit finish
intent can enter this profile. The pool cannot replace generated message bytes,
run inference, alter spent sampling work or substitute a smaller intent-envelope
charge for the original cumulative publication frame.

Both ordinary and generated pools provide `observe(driver, budget)` for an active
review, approval wait or terminal-reply grace. This operation takes neither a source
reader nor a clock callback. It services original polls, cancellation and recorded
submission retries, while withholding NEW submissions before the original port can
consume a waiting admission slot. The generated profile still runs its fixed intent
validator first. Recorded conflicts remain subject to exact original binding; a
known request ID is not permission to rebase it. An observation-only refusal does
not create a durable NotAdmitted record: a later explicit source-acquiring `drive`
may admit that same new ID. The operator must choose observation mode during review;
ordinary `drive` is not silently changed or automatically phase-switched.

This lets a supervisor keep cancellation and response traffic alive without
refreshing source evidence or admitting another effect while the original congress
owns its predecessor. `next_request` still refuses during an active driver job and
leaves its work hints intact. Cancellation runs through the original request and
control machinery; observation mode is not read-only with respect to those explicit
cancellation commands. A dispatched or published-but-unreconciled effect remains
Unknown until the ORIGINAL reconciliation acknowledges its actual outcome.

Two additional ordinary-port regressions check read-free observation, withheld-new
versus later-admitted controls, exact retry/conflict binding and cancellation after
source removal. Four generated-port regressions use the unchanged native-generator
fixture, actual sockets and original source/journal files. They cover malformed
references versus permitted generated admission, foreign-owner preflight, full-frame
charging, an active original helper-review job with read-free retry/cancel and
post-cancellation source repair, and the original two-key publication lifecycle.
The last requires the separately held human reviewer after the congress result;
it retains Unknown before reconciliation and exactly one charged visible message
afterward. The review uses an actual helper socket but does not execute helper
inference; the publication control supplies explicit reference ballots. Synthetic
weights and same-process sockets do not qualify a trained model or OS isolation.

There are now FIFTEEN authored regression functions for these two source additions
(nine initial pool tests, two observation tests, four generated-pool tests), plus
the authored no-authority compile-fail example. All remain UNEXECUTED. The second
required targeted RCH attempt also failed before compilation because `rch` is not
installed (exit 127). Rust compilation, formatting, Clippy and the full gate still
have no new execution evidence. No historical receipt is reused as qualification;
no Beads state or production gate is closed. The existing single-peer publication
CLI, runtime/dependency admission and deployment isolation are not changed here.
