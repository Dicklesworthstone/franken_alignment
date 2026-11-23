# Live Tower publication service and client

On Linux, the existing `supervise_publication` executable can serve ordinary
publication through the [Tower request schema](TOWER_REQUEST.md). The original
file owner, source admission, helper/human review, two publication keys,
independent Stop service and cleanup remain the execution path. The host fixes
the actor's grants before accepting a connection.

## Select the live protocol

Using an already qualified `supervise_publication` executable:

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41
supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --tower-generation 41
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --tower-generation 41
```

The configuration, actor/reviewer profiles and optional witness profile are the
existing [live actor service inputs](LIVE_ACTOR_SERVICE.md). The generation is
an explicitly supplied nonzero canonical decimal u64 identifying this route
registration. It is independent of policy epochs, native request generations,
journal revisions and clocks. It must match the generation in a proposal's
mounted run address. The host prints that address and the selected request's
poll address when it opens the listener.

For example, native tenant `1`, run `3`, principal `2`, branch `4`, authority `5`
and selected request `7` at registration `41` produce:

| Operation | Address |
|---|---|
| `propose` | `fa://1/run/3@41` |
| `get` after acquiring the ticket | `fa://1/attempt/request-3-2-4-5-7@41` |

These are examples; use the addresses derived from the actual configured scope.
The proposal's numeric request and `idempotency_key` must both be `"7"` in this
example. The complete target, payload, units, policy epoch and native deadline
still undergo original validation and review.

The four ordinary service modes above also accept an explicit bounded request
schedule, an operator endpoint, or a single-request precheck sidecar.
The two create modes additionally accept the existing topology-enforced
publication profile. The two open modes support explicit independently anchored
mediated recovery with a retained source checkpoint, as described below.
Unsupported combinations with multiple
peers, receipt-only services, native text or learned generation refuse before
opening their files. The grammar never silently selects a different protocol.
Existing native actor commands retain their current mode.

By default, the host grants only its configured tenant's control-ledger reads
and existing actor-intake purpose. An explicit `--precheck-profile` or a selected
`--precheck-schedule` entry also installs the original exact-predicate disclosure
described below. The actor session gains
no operator status, experiment context or supervisor authority. Unselected
operations return ordinary Tower refusals. A caller cannot add grants to JSON.

An explicit `--operator-profile OPERATOR_PROFILE` on an ordinary Tower
service provisions a separate authenticated endpoint for selected native
inspection, admission explanation, optional governance `ledger` reads and
`policy-replay`. The actor session keeps its original grants. The operator profile can compose with a
precheck sidecar, a finite request schedule, topology creation or explicit guarded
topology recovery, with distinct sockets and
independent budgets. A schedule retains the same operator session, native
custodians and spent allowances across every handoff. Use
`tower-operator-exchange OPERATOR_PROFILE JSON [JSON ...]` for explicit observations
and candidate policy comparisons.
Use `tower-situation OPERATOR_PROFILE RUN_ADDRESS --max-pages N` for one original
situation capture with bounded automatic continuation and explicit incomplete
output; every page is validated before its cursor is followed. Add a final
`--request-links` flag to validate the versioned original request addresses
alongside each captured attempt, with unchanged transport and query limits.
See the [live operator contract](LIVE_TOWER_OPERATOR.md) for the closed profile,
disclosure selection, request examples and Stop-first lifetime.

Governance accounting is disabled when the optional `governance_ledger` field
is absent or null. Selecting it requires 1–1,024 queries and explicit
`recorded_governance_accounting` disclosure. The existing
`tower-operator-exchange` client accepts `ledger` at the exact Run mount and
validates the complete response against the independently loaded operator
profile. The original query allowance survives reconnects and request handoffs;
the read acquires no source, clock, evaluator or publication key. See the
[governance ledger contract](TOWER_GOVERNANCE_LEDGER.md) for the request, original
accounting basis and bounded disclosure. The imported snapshot Tower does not
support this live read.

Policy replay is disabled when the optional `policy_replay` profile field is
absent or null. Enabling it requires explicit `policy_and_case_traces`
disclosure and bounded campaign, case and replay-input allowances. The service
installs the original owner's native replay grant and adds only `Experiment`
purpose to that operator endpoint. It adds no `Permit`, policy-promotion key or
actor admission capability. The report compares the complete original retained
policy corpus without fresh evidence, helper execution or policy mutation.

An exact campaign retry on a healthy reconnect returns the first historical
report and cut without another replay charge; a new campaign spends the same
grant's remaining allowance. Connection, exchange and publication-source work
remain independently bounded. Campaign custody and its cache survive request
handoffs within the service lifetime, and are revoked when the service retires;
the cache does not survive a service restart. See
[the operator replay example](LIVE_TOWER_OPERATOR.md#compare-a-candidate-against-the-original-retained-policy-corpus)
for the exact profile fields, request, disclosure and native limits.

An optional operator `response_bytes` selection raises the complete response
allowance from its unchanged 16,384-byte default to at most 1,048,576 bytes. It
uses the original native publication bounds and is validated before endpoint
setup; it grants no additional operation or disclosure. The client accepts
complete policy-replay traces with a separate finite 262,144-item JSON allowance,
while other operations keep their 4,096-item limit. Frame, string, nesting,
correlation and cumulative transport limits remain in force. A report that
still exceeds the selected output bound refuses whole, preserving its original
native cached campaign and spent work.

For complete reports that need multiple frames, explicitly select the optional
nested `policy_replay.page_queries` allowance, an integer from 1 through 1,024.
Absent means pages are disabled; zero or null is not an enabling selection.
An initial policy request with `"paged":true` returns a v2 header containing both
policies and the complete original summary. Issued continuations return at most
16 whole cases per page from that same cached campaign. Separate page charges
are cumulative across reconnects, campaigns and request-schedule handoffs;
exact original page retries neither replay nor replenish work. Existing complete
v1 requests stay unchanged.

Use `tower-policy-replay OPERATOR_PROFILE TOWER_REQUEST_JSON --max-pages N`
to validate and follow one explicitly paged initial request. The authenticated
client requires stable basis/policies/summary, exact contiguous case coverage,
unique ordered IDs, valid trace and Knowledge shapes, matching delta lists and
counts, issued cursors and explicit final completion. It emits only fully
validated original JSON lines and succeeds only after every case has been
received. Its bounded consumer retains IDs and header metadata, not all case
traces. A page cap, native exhaustion, damaged response or output failure stops
without automatic reconnect, retry, new campaign or refunded work. The original
frame, byte, deadline and I/O limits remain independent. See
[complete policy report enumeration](LIVE_TOWER_OPERATOR.md#enumerate-a-complete-policy-report-through-bounded-pages)
for exact request grammar, cache custody, validation scope and partial-output
semantics.

## Discover the live operator's selected grants

The separately authenticated operator endpoint accepts the existing
`capabilities` verb with exactly `"arguments":{"operator":true}`. Save this
complete request as `OPERATOR_CAPABILITIES_JSON`, using the actual printed Run
address and registration:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "inspect-selected-operator-grants",
  "address": "fa://1/run/3@41",
  "verb": "capabilities",
  "arguments": {"operator": true},
  "idempotency_key": null
}
```

```sh
supervise_publication tower-operator-exchange OPERATOR_PROFILE OPERATOR_CAPABILITIES_JSON
```

The host must already have selected `--operator-profile` and its native grants.
The profile's socket and kernel peer-credential checks remain required; the
request does not confer operator access. This mode also works through the
operator endpoints on the original learned and native generated services.
It does not enable actor exact-policy discovery on those restricted mounts or
provide ordinary actor intake.

The response kind is `operator-capabilities`, with closed schema
`fa.operator-capabilities/1`. Its Known `configuration` reports exactly fifteen
descriptors for the original supervisor roles, each with registered verbs,
required purpose, mount-route availability, caller-purpose availability,
installation status and native grant Knowledge. The
[native discovery contract](TOWER_PUBLICATION.md#discover-the-selected-original-operator-capabilities)
lists those roles and their routes. An installed grant can have a closed route
or disallowed purpose; none of these observations is a recommendation to act.

Known grant values carry the complete original native scope and acknowledged
journal revision, plus only the finite counters and limits explicitly named.
An exhausted grant remains Known with zero work. A revoked selected grant is
Withheld, while a missing installation is also Withheld with `installed: false`.
An unavailable, mutably borrowed, poisoned or replaced original owner is Unknown.
An equal-scope replacement cannot substitute its state for the old custodian.
Omitted limits and current execution eligibility remain explicitly
`not_observed`; `signed_profile` and `effect_authority` are always false.

Before emitting a complete reply, the client checks the closed schema, exact
fifteen-role ordering, route/purpose types, role-specific limit names, native
Knowledge variants, registration and request correlation. It additionally binds
every Known native scope to all six fields of the independently loaded operator
profile: tenant, principal, run, branch, authority and purpose. A response with
the right tenant/run but another principal, branch or authority is rejected.
Ordinary refusals remain refusals; relabeling a success or changing its declared
scope cannot bypass these checks. The client sends no automatic follow-up,
mutation, reconnect or retry after discovery.

Reading this configuration spends no original inspection, explanation,
resolution, policy, mutation, numerical replay or page allowance. It reads no
policy definition or source, consumes no actor admission snapshot and changes
no native state. The live connection, exchange, frame, byte, I/O and deadline
bounds still apply. A successful exchange means the explicit observation was
validated, not that a listed operation is currently permitted. Current source,
review, predecessor and publication checks remain with the original operation.

## Preserve topology enforcement through Tower admission

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41 --topology-profile TOPOLOGY_PROFILE
supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --topology-profile TOPOLOGY_PROFILE --operator-profile OPERATOR_PROFILE --tower-generation 41
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41 --topology-profile TOPOLOGY_PROFILE --requests 7,8
```

The topology, generation, optional schedule and operator pairs may follow the positional
files in any order. This selects the original
[topology-supervised creation workflow](LIVE_TOPOLOGY_SERVICE.md) and, for the
checked form, its [plain publication-witness contract](LIVE_CHECKED_TOPOLOGY_SERVICE.md).
The actor still uses `tower-exchange` with the printed registered run address.

The same mediated bootstrap installs the policy-source guard, topology,
publication guard, terminal reserve and selected witness limits in the first
canonical image. Before admitting an actor request, the original bounded source
refresh must certify the modeled cut. Changing the request encoding does not
change helper review, independent human approval, either publication key, source
freshness or the native effect gate.

The driver's final publication path refreshes topology again after policy
capture. A bypass, changed or missing topology input, exhausted source allowance
or failed required witness can still prevent the effect after human approval
and dispatch. Original unresolved-outcome and charged-rights rules apply; a
failed observation does not refund an unknown effect. Unrelated witness changes
can pass only through the existing semantic witness checks.

Version 2 topology profiles attempt their selected checkpoint output from the
driver's actual custody after execution, Stop and reply processing, including
acknowledged failures. Native validation must still establish a healthy exact
owner/source cut. A failed later request cannot silently discard custody for
earlier committed effects or spent source work. Helper cleanup runs
after retention and can still make the command fail. A retained checkpoint is
acknowledged source/history evidence, not proof that all direct children were
reaped. Actor, human-review, Stop and operator sockets
and checkpoint destinations must be distinct, including canonical parent-path
aliases; conflicts refuse before clock or persistent setup. Operator observations have
only their explicitly selected native grants and cannot certify
a cut or advance publication.

An acknowledged source refusal is retained with its actual unavailable state,
original documents and spent allowance; it does not certify the rejected cut
or refund work. The original service error remains an error, with independent
checkpoint failures appended. Capture performs no live-source reread, clock
observation, refresh or effect retry. Failure before native custody exists
produces no checkpoint, and an unhealthy owner cannot export one.

An explicit `--requests` schedule uses the same original mediated owner, driver
and topology source throughout. Its first qualification and every later final
publication check spend the original cumulative source allowance. Every request
still acquires fresh actor evidence, helper review and human approval; a prior
certified publication cannot certify its successor. Handoff waits for terminal
settlement and confirmed helper reaping. Actor tickets, operator queries,
connection/exchange limits and deadlines remain cumulative. Canonical destination
checks cover every future reviewer and Stop socket before setup, including any
selected checkpoint archive.

This creation composition uses `serve-create` and `serve-create-checked` with
plain witness profiles. Single-request Tower creation can also select
`--precheck-profile` as described below. Topology cannot be combined with
ordinary unguarded open/recovery, credibility activation, multiple
peers, native text or generated/learned modes. Feed, wait, snapshot and joint
witness profiles are refused before clock, source, socket or store setup.
Recovery requires the existing independently retained mediated history and
source-work contract. Certification remains conditional on the supplied graph
and inventory; this integration does not establish complete host containment.

## Reopen with exact mediated recovery custody

```sh
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41 --guarded RECOVERY_PROFILE --topology-profile MEDIATED_SUBMIT_V2_PROFILE
```

`serve-open-checked` accepts the same options after its plain witness profile;
both support an explicit `--requests` schedule and the separate operator endpoint.
For one new request, either open mode can instead select `--precheck-profile`.
That disclosure belongs to the recovered original owner and uses its current
policy epoch after recovery fencing. Recorded requests, schedules and a live
governor cannot acquire this single-request precheck selection.
The explicit `fa.guarded-submit/1`
history/floors and `fa.mediated-submit/2` source checkpoint must describe the same
complete original cut. Recovery reuses the original native Fence/Time/Sweep,
keeps source spending, refuses unresolved old effects and requires newer graph
and inventory generations before fresh work. The same source, helper, human,
two-key and final topology/witness checks determine publication.
One original owner and cumulative source allowance span the entire recovered
schedule. A retained prefix can precede fresh requests; entirely retained
schedules acquire no live source or helper assets. Each handoff still waits for
terminal settlement and confirmed helper reaping.

Recorded requests remain observational and can retain a new paired checkpoint
without live source files or helper programs. Authenticated Stop stays available;
later Stop mutation cannot be represented by an obsolete source checkpoint.
See [the live recovery contract](LIVE_TOWER_RECOVERY.md) for grammar, custody,
elapsed boundaries, failure retention and verification limits.

## Check exact policy before a Tower proposal

The single-request forms can explicitly select the existing authenticated
[native precheck service](LIVE_ACTOR_PRECHECK.md):

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41 --precheck-profile PRECHECK_PROFILE
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --precheck-profile PRECHECK_PROFILE --tower-generation 41
```

Both options follow the positional arguments and may appear in either order.
The same composition is available for `serve-open` and `serve-create-checked`.
Select one of the existing uncached, cached or full-policy precheck profiles;
its original disclosure, socket, policy epoch, query and lifetime limits apply.
`--requests` cannot be combined with this option: disclosure belongs to the
single selected admission and is never renewed at a sequence handoff. A live
governor is also incompatible with this selection.

The create forms can additionally retain their original topology supervision:

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41 --topology-profile TOPOLOGY_PROFILE --precheck-profile PRECHECK_PROFILE
supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --precheck-profile PRECHECK_PROFILE --topology-profile TOPOLOGY_PROFILE --tower-generation 41
```

Guarded recovery can select the same disclosure for one new request:

```sh
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE --guarded RECOVERY_PROFILE --topology-profile MEDIATED_SUBMIT_V2_PROFILE --precheck-profile PRECHECK_PROFILE --tower-generation 41
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --tower-generation 41 --precheck-profile PRECHECK_PROFILE --guarded RECOVERY_PROFILE --topology-profile MEDIATED_SUBMIT_V2_PROFILE
```

All option pairs follow the positional files and can appear in any order. These
topology compositions support one ordinary request and, for checked modes, the
existing plain witness profile. Recovery requires the independently retained
guarded history and exact `fa.mediated-submit/2` source checkpoint; its selected
request must be absent from the recovered journal. Set the precheck profile's
expected policy epoch to the recovered owner's epoch, including recovery fences.
The single `--precheck-profile` option cannot be combined with a schedule or
live governor. This ordinary Tower composition also excludes native text,
generated/learned owners and
stronger witness profiles.

Initial topology certification completes before the precheck endpoint is
exposed. The same original source custodian pays the final check before
publication; a precheck query performs no topology refresh and a precheck pass
cannot stand in for a certified final cut. The actual submission still obtains
fresh policy evidence and both independent publication keys. Version-2 topology
profiles retain their actual acknowledged source/history checkpoint on normal
or failed exit. The checkpoint destination and precheck socket must be distinct,
including aliases through canonical parent directories; conflicts refuse before
time, store or source admission.

The selected profile installs a weak exact-predicate port on the existing actor
Tower session. Its `precheck` and `capabilities` verbs use the printed run mount
and the [Tower request schema](TOWER_REQUEST.md). The actor can send the complete
explicit sequence through one authenticated connection:

```sh
supervise_publication tower-exchange ACTOR_PROFILE TOWER_CAPABILITIES_JSON TOWER_PRECHECK_JSON PROPOSE_JSON GET_JSON
```

The client sends each supplied document in order; it does not condition a later
proposal on an earlier precheck result. To choose a proposal after inspecting the
observations, send only the read documents first, then explicitly submit the
chosen proposal. Reconnection does not replenish the original disclosure.

The existing `actor-precheck ACTOR_PROFILE PRECHECK_PROFILE PRECHECK_JSON` command
continues to use the separate native socket. Both protocols authenticate against
the original actor peer policy and share one cumulative native query allowance,
including discovery, repeated labels, cache hits and reconnects. Their configured
transport connection and exchange limits remain separate. The sidecar retains
the sole disclosure custodian; the Tower copy cannot extend its lifetime or
issue a replacement grant.

For a full-policy profile, the native endpoint retains its explicitly selected
policy-definition access. The Tower receives only the narrowed exact-predicate
port: its `capabilities` response reports `policy_contents: "withheld"`, and it
does not expose policy structure or literals. Uncached and cached profiles keep
their original witness-cache behavior and disclosure boundaries.

The original supervisor bootstraps the source once before exposing precheck.
Queries and reconnects share the same finite allowance and source lease. A
passing precheck still requires fresh source acquisition at Tower submission,
independent helper/human review and both publication keys. Checked service modes
also keep their final-publication witness checks. A changed source or witness
can therefore prevent publication after an earlier precheck pass.

Independent Stop is checked before either ingress. A malformed native sidecar
frame or an original sidecar lifetime/transport limit retires its disclosure,
revoking the Tower copy while ordinary Tower work remains available. Recording
the selected request, entering review, an owner change and every service exit
also retire the original grant. A recorded request cannot acquire a new precheck
grant during recovery. Socket collisions and invalid combinations refuse before
source acquisition or store creation.

## Publish an explicit request sequence

Add `--requests` to any of the four Tower service modes:

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41 --requests 7,8
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --requests 7,8 --tower-generation 41
```

Both options follow all positional arguments and may appear in either order.
The schedule contains one to 64 distinct nonzero canonical decimal u64 keys in
execution order. Its first key must equal the actor profile's `request`.
Malformed schedules refuse before configuration reads; all scheduled actor,
reviewer and Stop socket bindings are checked before creating the actor socket
or publication store.

Ordinary `serve-create` and `serve-create-checked` with the plain witness profile
may additionally select `--precheck-schedule PRECHECK_SCHEDULE`. This create-only
manifest names a profile or explicit `null` for every request; all profiles,
cumulative disclosure bounds and future socket collisions are checked before
startup. Each selected entry's native socket and existing Tower session share
one finite native query allowance. Each entry's disclosure retires before
review; a later handoff installs only its explicitly selected grant, and a null
entry inherits none.
The original topology and operator options can also be selected. This schedule
option excludes open/recovery, governed services and `--precheck-profile`;
the existing guarded single-new-request recovery option described above remains
available. See [scheduled actor prechecks](SCHEDULED_ACTOR_PRECHECK.md) for the
closed grammar, profile selection and grant lifecycle.

The service uses the original [sequential actor lifecycle](SEQUENTIAL_ACTOR_SERVICE.md)
with one authority owner and one Tower session. Tickets and cumulative candidate,
connection and exchange allowances survive every handoff. Rights, journal
capacity, recovery reserves and service deadlines retain their original limits.
Only the currently selected new request can acquire fresh source evidence.
Earlier requests remain available for exact retries and ticket-scoped reads;
conflicting retries still compare the complete retained proposal. Future or
unlisted proposals cannot acquire a snapshot or enter a speculative queue.

Each admitted publication still requires the original helper review, independent
human approval, both publication keys and receipt settlement. Checked modes also
retain their original witness guard. Before advancing, the driver verifies the
prior request's terminal disposition and confirms direct helper children were
reaped. Pending review, dispatching, Unknown and irrecoverably Unknown liabilities
cannot be supplied as a completed prefix. An unfunded proposal is retained as
`NotAdmitted(Limit)` by original admission and does not launch review or refund
earlier spending.

Send the next proposal after the current result is terminal and use its actual
current target version and policy epoch. Independent Stop remains available while
waiting for each request and through the original execution checkpoints; the
service handles at most one complete actor frame per turn. Source failure on a
later request cannot reuse earlier evidence. Opening a recorded schedule follows
the original recovery fence: historical retries do not acquire source evidence,
launch helpers, repeat human approval or open an activation capsule. Every newly
selected request must still satisfy current admission and qualification.

## Exchange explicit request documents

```sh
supervise_publication tower-exchange ACTOR_PROFILE PROPOSE_JSON GET_JSON
```

This actor-only command accepts one to 64 complete Tower documents and emits
one original complete response per document as newline-delimited JSON. It reads
the existing public actor connection profile, validates and freezes the entire
document sequence before connecting, and verifies the connected server's kernel
credentials before sending bytes. Pretty-printed input is canonically encoded
into one frame; identifiers, integer values and payload bytes are preserved.

The client checks each response's bounded JSON syntax, exact common envelope,
schema, operation/address/key correlation and native result profile. The body
remains an opaque observation. A successful client exit means that its explicit
exchanges completed, not that a proposal was approved or an effect executed.
`Pending`, `Unknown`, native refusals and terminal outcomes remain present in
the original returned bytes.

There is no implicit polling, reconnection, replacement key or automatic resend.
Supply `get` documents explicitly. A lost response leaves the original request
identity in force; reconnecting clients retry the same complete proposal to
recover its ticket. A new process or Tower session cannot acquire a ticket by
guessing a poll address. An exact retry after original durable recovery compares
the full retained proposal and does not reread an unavailable evidence source.

The client's elapsed deadline and cumulative byte/I/O-attempt budgets cover all
documents. Short reads/writes and interruptions retain their original progress;
output failure stops before sending another document. Unterminated, oversized,
uncorrelated or unsolicited replies are not emitted as complete results.

## Library integration and scheduling

`tower::publication::peer::PublicationPeerSession` combines an original
`PublicationTower`, fixed host-created `TowerAccess`, the existing `PeerPolicy`,
`ChannelLimits` and a finite connection count. It authenticates every candidate
through the existing Linux UID/GID and optional PID gate before reading bytes.
This identifies the kernel-observed connecting process, not executable code,
a human or a remote identity.

The session exposes `attach`, `status`, `socket_fd`, `disconnect`, `revoke` and
`remaining_exchanges`. The trusted original driver uses
`drive_tower_request_from_file` during selected admission and
`drive_tower_request_observe` during review/recovery. The former acquires fresh
source/time only for a new valid selected proposal; the latter accepts neither
source nor clock and cannot admit unrecorded work. Both retain original helper
maintenance and keep source preparation diagnostics outside actor response bytes.

The host can install an existing disclosure with
`PublicationPeerSession::set_precheck(Some(port))`, including while a response
is pending. It delegates to the original Tower's owner-allocation and complete
scope checks. Replacement preserves the pending response, request tickets,
connection count and spent exchange/query allowances; a foreign replacement
withdraws the previous installation before refusing. `set_precheck(None)` removes
only this facade's copy. A revoked peer rejects installation, and the native
port continues to check issuer identity, custody, revision and source eligibility
on every query. The method creates no grant and retains no strong owner or
disclosure-custodian reference.

For a bounded schedule, use `drive_tower_sequence_from_file` and
`drive_tower_sequence_observe` with the current key and completed prefix. Both
check the exact original owner and the original journal's terminal dispositions
before socket I/O. They share the native sequence's predecessor validation and
the same peer session; the completed list is routing data, not settlement evidence.

The socket follows the existing `DriveBudget` / `DriveReport` scheduling contract.
It holds one bounded input frame, one reply and an 8 KiB unread socket buffer.
Input stops at the first LF. A buffered next frame cannot execute until the prior
reply is completely written **and flushed**. Every attempted socket operation
spends an I/O slot, including interruption and would-block. Buffered local work
and pending flushes remain schedulable without waiting for another read event.
The ordinary host admits at most one complete frame per turn, preserving its
independent Stop checkpoints even when a client sends a fragment or pipeline.

| Resource | Bound |
|---|---|
| Tower request frame | Host-selected, at most 256 KiB, excluding LF |
| Session exchanges | Host-selected, at most 4,096 over all reconnects |
| Authenticated connections | Host-selected, at most 1,024 |
| Concurrent sockets | One per session |
| Native response | Original publication limit: 16 KiB default, at most 1 MiB |
| Socket work per drive | Existing byte, frame and I/O-call budgets |

Malformed complete frames spend an exchange; incomplete EOF or over-limit
frames never execute. An encoding failure after native admission also spends
the entered exchange and closes transport without retrying or undoing the
request. Reconnection discards incomplete transport bytes and preserves the
same tickets, disclosure grants and spent lifetime allowance. Revoking ingress
closes transport only; stopping, cancelling or settling effects requires the
original supervisor transition. The peer retains no strong owner reference.

## Qualification scope

Regression sources cover real peer credentials, source-backed submission,
buffered-frame backpressure, lost replies, reconnect limits, foreign owners,
owner loss, actual helper/human publication, observation after recovery without
source, and independent Stop while a client holds a partial frame. Client tests
cover correlation, strict grammar, short I/O and bounded failure handling.

The sequential increment adds thirteen regression functions: five native socket,
six complete service and two option-grammar cases. They cover two real
publications, original cumulative rights, missing later source, historical
recovery, partial-frame Stop, unretired liabilities, foreign owners and schedule
preflight. One existing native budget regression is corrected to match original
pre-review admission. These new and corrected cases have not executed.

Qualified syntax/format and source review are separate from execution evidence.
RCH is unavailable in the current workspace; compilation, Clippy, runtime tests
and the full canonical gate remain pending for this increment. This service
does not claim the complete prospective `fa` CLI, all Tower verbs, learned
publication transport or signed receipt verification.

The earlier native-sidecar composition added nine authored regression functions,
including the three disclosure profiles, shared work across reconnects, real
two-key publication, changed source/final witness, partial-frame Stop, socket
preflight and recorded-request refusal. Source review and a Rust 2024
syntax/format pass do not establish execution: RCH has no configured workers and
the required remote command refuses with an unavailable daemon. Compilation and
runtime qualification remain open.

The same-session disclosure extension adds six regression functions and
strengthens three existing live-service cases. They cover shared work across
both protocols and all three profiles, full-policy narrowing, read-only queries
with an absent source file, exhaustion/reconnect, original deadline retirement,
missing grants, invalid and stale requests, pending-response preservation,
foreign-owner replacement, revocation, changed-source denial and independent
Stop. Centralized Rust 2024 parsing/formatting checks and source review completed;
compilation, Clippy, runtime execution and the canonical gate remain pending
because the required RCH service is unavailable.
