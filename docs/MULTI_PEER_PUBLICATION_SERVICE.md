# Multi-peer publication service

## Source addition and change log

The existing Linux `supervise_publication` service now consumes the original
`FileActorPool`, rather than leaving multi-peer intake as an unused library API.
This is the L5 ingress consumer for FA-107 and plan sections 9.3, 15 and 17.1.
It adds no actor verb, authority domain, dependency, journal tag or wire format.
The existing single-peer and explicit `--requests` modes are unchanged.

Use the existing strict actor profiles, each with its own socket, credential
policy and distinct request ID. The primary profile plus the additional profiles
must total 2..=16 peers, all in the same original supervisor scope:

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  serve-create CONFIG ACTOR_ONE REVIEWER --peers ACTOR_TWO ACTOR_THREE

cargo run --locked -p fa-reference --example supervise_publication -- \
  serve-open-checked CONFIG ACTOR_ONE REVIEWER WITNESS_PROFILE \
  --peers ACTOR_TWO ACTOR_THREE
```

`serve-create-checked` and `serve-open` accept the same option. Existing open-mode
`--credibility-activation EVIDENCE_FILE` still uses the original qualification
path, at most once per service lifetime and only when new work is possible.
Do not combine `--peers` with `--requests`. Actor clients use the unchanged
`actor-submit ACTOR_PROFILE SUBMIT_JSON` command. Every document still names its
own exact target version, policy epoch, deadline and original request identity;
queued proposals are never rewritten or rebased.

## Ownership, progress and stop

All profiles, scopes and cross-role path collisions are checked before socket
or store creation. All actor listeners bind before the durable owner opens.
A partial bind failure releases only newly owned socket identities; existing
files and stale/foreign sockets are never removed. Every endpoint constructs a
fresh original inbox and uses `FileActorPool::for_requests`. The registered path,
not an actor frame, selects the session and its fixed request. Kernel credentials
are checked before any actor bytes. Candidate and connection limits remain per
original session; the socket work allowance is shared across the pool.

At most one complete frame is processed per source-acquiring intake turn. Its
actual journal disposition is checked before another new request can enter.
Ready selection is independent of profile order: an absent or incomplete first
peer need not prevent another peer from reaching the original review workflow.
The original process helpers, full submitted-input binding, separate human key,
checked publication, reconciliation and helper retirement are reused unchanged.
No second congress, automatic human approval or alternative effect ledger exists.

During review, approval waits and helper retirement, `observe_registered` services
only peers whose bound request is already recorded. NEW peers remain unread,
using ordinary socket backpressure instead of receiving a transient Withheld
response that would require a resend. Their exact queued frame is admitted on a
later source-acquiring turn. Recorded peers retain cancellation, polling and exact
retry behavior without source reads or a clock. An unrestricted pool cannot use
this optimization because its peer names do not identify all possible requests.
The original `observe` API and its explicit refusal behavior remain available.

One stop endpoint persists for the whole live service. Its operation ID is the
FIRST actor profile's request, even when another peer's request executes first.
Use the existing `stop-peer REVIEWER FIRST_REQUEST_ID` command. Internal shared
handles borrow one original transport: socket identity, credentials, candidate
quota and sticky terminal outcome are not copied or renewed for each review.
Reentrant driving refuses rather than permitting two mutable protocol owners.
The separately held reviewer role and all real stop transitions remain original.
The shared runtime and reply grace use the smallest corresponding actor-profile
limits (runtime also intersects the supervisor limit); polling uses the smallest
actor poll interval. New admission waits for original helper-child retirement.

## Recovery and interpretation

Opening uses the same guarded, fenced native owner. Already recorded keys are
reconciliation/receipt-only: they never launch new helpers or obtain new approval.
A fully historical service reads no evidence/qualification file and creates no
stop or review listener. It waits for an actual complete frame from each peer
before starting its bounded reply grace; a received frame is not a claim that a
terminal response was delivered. The original wire and ticket checks still apply.

A successful service return means its bounded workflow finished or an original
stop completed, not that every request executed. Each actor must inspect its own
original Knowledge result. Unknown outcomes are never execution confirmation;
failures use the original stop/drain path and retain unconfirmed liabilities.
The private intake reports never become actor responses.

## Validation status and boundaries

Seven `multi_peer_` example regressions are authored. They exercise option bounds,
whole-profile/cross-role preflight, partial socket-setup cleanup, an incomplete
first frame alongside a permitted second admission, two full publication workflows
in reverse profile order with separate approvals, source/helper-free replay of
both exact requests, shared stop-socket lifetime, and an actual independent stop
while all actors and source evidence are absent. The two-publication test checks
original executions, exact final bytes, charged units and zero remaining reserves.
It uses the existing synthetic helper executable and explicit reviewer choices,
not trained detector inference or an independent human-identity evaluation.

An additional `scoped_pool_` regression verifies unread-new-peer backpressure,
concurrent owner cancellation, and later admission of the SAME queued frame
without a resend; unrestricted mode is a refusing control. Together with the
preceding request-binding commit there are five scoped-pool and seven example
regressions. Existing test bodies and the synthetic helper fixture are unchanged.

Both targeted RCH commands and the full gate were attempted. RCH is absent, so
each attempt failed BEFORE compilation with exit 127. Compilation, all twelve
new tests, rustfmt, Clippy and the full gate remain UNEXECUTED. Source/whitespace
and uploaded-byte checks are not runtime qualification. No Beads closure or
production release claim follows.

The CLI currently serves the original ordinary publication port; generated-text
ports retain their existing library integration, not a new decoder-serving CLI.
All peers share one authority scope. This does not add multi-tenant dispatch,
Asupersync/dependency admission, hostile-process containment, remote identity,
hardware crash qualification or a hard real-time filesystem deadline. Socket
credentials identify connection-time UID/GID/PID, not executable code or a
transferred descriptor's current holder. Source and clock authenticity retain
the existing operator assumptions.
