# Live operator inspection through the original Tower owner

On Linux, ordinary single-request Tower publication services can expose a
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

All four ordinary single-request Tower serve modes support the option. The
generation, operator and optional precheck pairs may appear in any order after
the positional files. An operator profile requires an explicit Tower generation;
request schedules, multi-peer, topology, native-text and learned service modes
do not consume it. Unsupported combinations fail before reading configuration.

An optional `--precheck-profile PRECHECK_PROFILE` retains its separate actor
disclosure and socket. The operator, actor, precheck, human-review and Stop
paths must be distinct. Directory ownership, the selected audience and socket
collisions are checked before a clock read, source acquisition or store creation.
Existing socket paths are never unlinked to make setup succeed.

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

## Lifetime and authority

The service provisions the original native read grants once and retains their
noncloneable custodians. One Tower session and its continuation state survive
healthy reconnects. Reconnection, repeated operation labels, review transitions
and reply grace cannot replenish query, candidate, connection or exchange work.
The operator's elapsed lifetime is bounded by its profile, actor lifetime and
the remaining wall lifetime of the original supervisor deadline.

The host pumps at most one complete operator frame per cooperative checkpoint,
after independent Stop. It continues during helper/human review and terminal
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
