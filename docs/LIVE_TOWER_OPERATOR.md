# Live operator inspection through the original Tower owner

On Linux, ordinary Tower publication services can expose a
separate authenticated operator endpoint. It connects the existing native
`situation` and historical admission `explain` capabilities to the running
supervisor. The service keeps its original journal lock, source admission,
helper/human review, publication keys, independent Stop and cleanup path.

## Select the operator endpoint

Using an already qualified executable:

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --tower-generation 41 --operator-profile OPERATOR_PROFILE
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --operator-profile OPERATOR_PROFILE --tower-generation 41
```

All four ordinary Tower serve modes support the option for one request or an
explicit finite request schedule. The generation, operator and selected
schedule/precheck pairs may appear in any order after the positional files.
The two create modes can instead select the existing topology profile alongside
the operator endpoint. Both open modes can compose explicit guarded mediated
recovery with an exact v2 source checkpoint and the operator endpoint. An operator
profile requires an explicit Tower generation.
Multi-peer, native-text and learned service modes do not consume it. Unsupported combinations
fail before reading configuration.

An optional `--precheck-profile PRECHECK_PROFILE` retains its separate actor
disclosure and socket. The operator, actor, precheck, human-review and Stop
paths must be distinct. Directory ownership, the selected audience and socket
collisions are checked before a clock read, source acquisition or store creation.
Existing socket paths are never unlinked to make setup succeed.

Create-only `--topology-profile TOPOLOGY_PROFILE` keeps the original mediated
bootstrap and final topology refresh, including plain checked publication.
Operator reads cannot certify the cut or replace either publication key. A
version 2 topology checkpoint destination must not alias the operator socket;
this is checked before clock, source or persistent setup. Topology cannot select
precheck, a request schedule or ordinary open/recovery. See the
[Tower topology contract](TOWER_PEER.md#preserve-topology-enforcement-through-tower-admission).

Explicit `--guarded RECOVERY_PROFILE --topology-profile MEDIATED_SUBMIT_V2_PROFILE`
on the Tower open modes preserves the original owner and source-work custody
before installing operator grants. It supports fresh checked publication and
recorded observations without letting operator traffic refresh source evidence
or start a new request. See [live mediated recovery](LIVE_TOWER_RECOVERY.md) for
exact paired custody, independent Stop and failure-retention boundaries.

## Observe a finite publication sequence

```sh
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --requests 7,8 --operator-profile OPERATOR_PROFILE --tower-generation 41
```

The original bounded schedule retains one operator session and its native grant
custodians for the entire owner lifetime. Selecting the next request does not
reissue inspection or explanation work, clear a pending continuation, reset
transport allowances or extend elapsed deadlines. The operator can inspect an
earlier confirmed request while a later request waits for its independent human
approval. A continued situation still reads its original immutable capture;
an explicit fresh situation captures the advanced owner.

Preflight checks the operator listener against **every scheduled** human-review
and Stop path before the first clock read or store/socket creation. A late key
cannot collide with a listener that was valid only for the initial request.
All original source, review, publication, settlement and confirmed helper-reaping
conditions still govern handoff. Operator reads cannot advance the schedule.

Read limits spent on an earlier request remain spent during later requests,
reconnects and final reply grace. A fully recorded schedule can expose its
historical observations without source files or helper programs, using the
original recovery path. Request schedules still cannot select the single-request
precheck sidecar; it has a different admission-time disclosure lifetime.

## Public operator profile

The profile fixes the peer identity and finite read allowances. It contains no
private supervisor configuration, source path, helper recipe or publication key.
For example, replace the scope, paths and kernel identities with the actual
deployment's values:

```json
{
  "schema": "fa.tower-operator-service/1",
  "scope": {"tenant": 1, "principal": 2, "run": 3, "branch": 4, "authority": 5},
  "socket": "/run/fa/operator.sock",
  "supervisor": {"uid": 1000, "gid": 1000, "pid": null},
  "operator": {"uid": 1001, "gid": 1000, "pid": null},
  "candidate_limit": 8,
  "connection_limit": 4,
  "exchange_limit": 128,
  "runtime_ms": 60000,
  "poll_ms": 5,
  "inspection_queries": 64,
  "explanation_queries": 8,
  "explanation_replay_events": 4096,
  "explanation_disclosure": "policy_and_selected_witnesses"
}
```

The schema is closed: missing, duplicate or extra fields fail. Profile integers
are unsigned JSON integers, as in the existing actor profiles. Tower request
integers retain their separate canonical decimal-string format.

Inspection and explanation have separate allowances, each bounded by the
original native maximum of 1,024 queries. At least one must be enabled. For
inspection only, set `explanation_queries` and `explanation_replay_events` to
zero and `explanation_disclosure` to `"none"`. Enabling explanation explicitly
selects policy-literal and selected-witness disclosure, with a positive bounded
replay-event allowance. It does not disclose the entire source, helper input
or action payload.

The configured supervisor identity and complete scope must match the actor
service. The socket directory must be supervisor-owned and not group/other
writable; the owned socket receives mode `0660`. The operator identity is
independent of the actor and reviewer profiles. Each endpoint checks actual
Linux UID/GID and optional PID before reading a byte. These are process kernel
identities, not executable or human authentication. If profiles name the same
kernel identity, those processes share the explicitly granted access.

## Exchange original Tower documents

```sh
supervise_publication tower-operator-exchange OPERATOR_PROFILE SITUATION_JSON EXPLAIN_JSON
```

This client reads only the public operator profile and one to 64 explicit Tower
request files. It freezes and validates the whole request sequence before
connecting, authenticates the server before sending bytes, and shares the
original Tower client's elapsed, byte and I/O-attempt limits. Replies remain
their original complete correlated JSON observations, one per line. There is
no automatic poll, reconnect, retry or implicit submission.

An initial situation request for the example scope and generation is:

```json
{
  "schema_version": "fa.tower_request/1",
  "operation_id": "operator-situation-1",
  "address": "fa://1/run/3@41",
  "verb": "situation",
  "arguments": null,
  "idempotency_key": null
}
```

Further pages use the returned opaque continuation according to the
[native inspection contract](TOWER_PUBLICATION.md#inspect-the-original-live-owners-situation).
They preserve the original frozen capture even when the live owner advances.
An explicit request for `explain` uses the original request address printed by
the actor service, for example `fa://1/attempt/request-3-2-4-5-7@41`, with
`arguments: null`. It describes the original policy admission and its distinct
ending cut; it does not establish current eligibility or explain a later hold.

To obtain those original request addresses from the situation itself, explicitly
select `{"request_links":true}` and use the same flag alongside each returned
cursor. The resulting `fa.publication-situation/2` rows include paired nullable
`external_request` and `request_address` fields. They are frozen with the original
capture and preserve full request scope and registration generation. Default
situation requests keep their existing v1 shape. A link can be supplied in a
separate explanation request only when that endpoint already has the independent
explanation grant. See [the request-link contract](TOWER_PUBLICATION.md#follow-captured-attempts-to-original-requests).

## Enumerate one original situation capture

```sh
supervise_publication tower-situation OPERATOR_PROFILE fa://1/run/3@41 --max-pages 64
supervise_publication tower-situation OPERATOR_PROFILE fa://1/run/3@41 --max-pages 64 --request-links
```

This Linux command follows the native situation continuation for you. The run
address must be canonical, match the operator profile's tenant and run, and
include the exact nonzero registration generation. The explicit page allowance
is a canonical integer from 1 through 64. Grammar and address checks precede
profile reads; profile/run checks precede connection. The server authenticates
the operator and the client authenticates the server before protocol bytes.

The default first request has null arguments. Later default requests use only
the exact cursor returned by the previous accepted page, on the same connection and within one
transport budget. The smaller of `--max-pages` and the profile's original
exchange allowance limits requests. Native query exhaustion remains independent
and can end enumeration earlier. No reconnect, retry or new capture occurs.

The optional final `--request-links` flag explicitly selects v2 on every page:
`{"request_links":true}` initially, then the same flag with the exact returned
cursor. Default mode requires the original v1 response; linked mode requires
v2. An unexpected profile or a schema relabeling refuses before output or
continuation, with no automatic fallback.

Linked rows have exactly ten fields, including paired nullable
`external_request` and `request_address`. The client verifies each canonical
nonzero full-width key against the exact address derived from the original
scope and selected registration generation. Tenant, run, principal, branch,
authority, generation and key must all match. Direct attempts require both
fields to be null. Duplicate request identities within/across pages and more
than the original 128 request keys refuse; direct attempts do not consume this
key limit. The entire page is checked before retaining any new IDs or capture
progress. Returned links remain data for separately authorized explicit reads;
the client does not issue Explain or Get calls for them.

Before emitting or following a Known page, the client checks the original
acknowledged-owner basis, complete scope, unchanged journal/control/time cut and
whole-ledger summary. It also checks contiguous offsets, the native 16-row page
bound, unique attempt IDs and continuation IDs, canonical full-width integers,
accounting totals and explicit completion. A page whose capture basis or summary
changes, or whose sequence is inconsistent, is rejected before its bytes are
output. The server's original branded native continuation preserves the actual
capture identity; live owner changes do not replace that capture.

Original accepted replies are written as JSON lines. Exit success means the
Known enumeration explicitly completed; it does not mean the observation is
current eligibility or publication authority. If a page, exchange or native
grant limit ends an incomplete enumeration, already emitted valid pages remain
available and the command exits with an error. Correlated native Unknown,
Withheld, refusal and error replies are also preserved before failure. Output
failure stops further requests without restoring spent native work; a failed
write can leave partial output, so consumers must check both framing and exit
status.

The client retains one basis and summary plus at most 1,024 attempt IDs and 64
cursor IDs, with at most 128 external keys in linked mode, rather than accumulating the entire ledger or all page bytes. A
larger reported total is valid but does not enlarge this work allowance. The
existing client checks elapsed time and finite byte/I/O work at checkpoints;
synchronous OS connection and output calls are not preempted by those checks.

Nine additional authored client regressions cover linked native captures across
live owner changes, full-width/direct identities, explicit v1/v2 selection,
scope/generation/key substitution, nullable pairs and row closure, duplicate
keys with atomic rejected-page handling, no follow-up after a damaged reply,
independent limits, the actual 128-request plus direct-attempt boundary and
closed final-flag grammar. Independent source review and syntax/format checks
remain separate from the pending remote compilation and runtime gate.

## Lifetime and authority

The service provisions the original native read grants once and retains their
noncloneable custodians. One Tower session and its continuation state survive
healthy reconnects. Reconnection, repeated operation labels, review transitions
and reply grace cannot replenish query, candidate, connection or exchange work.
The operator's elapsed lifetime is bounded by its profile, actor lifetime and
the remaining wall lifetime of the original supervisor deadline.

The host pumps at most one complete operator frame per cooperative checkpoint.
Independent Stop runs first at active intake and execution checkpoints. Reads
continue during helper/human review and the original observation-only terminal
reply grace. Truncated, oversized or failed operator transport retires that
sidecar without entering the publication workflow's mutation or Stop path.
Every exit retires the sidecar before helper cleanup. The native read grants
retain no strong owner or authority lock, and their original health/custody
checks still apply to every query.

Only the separate operator session receives operator read access. It has no
Permit purpose, source callback, precheck disclosure, helper/human key or
supervisor mutation capability. A proposal sent to it cannot acquire source
evidence or create an action. The actor endpoint keeps its original grants and
cannot obtain these reads by adding fields to a request. Opening an existing
recorded request can provide historical operator reads without reacquiring
source, launching helpers or repeating approval.

These are cooperative observations of the original acknowledged state. Native
synchronous source, journal and replay work is not preempted by this endpoint.
The feature does not supply the full prospective `fa` CLI, native current
`why-held`/`next`, mutation verbs or deployment qualification. Compilation and
runtime qualification remain pending the required remote execution gate.
