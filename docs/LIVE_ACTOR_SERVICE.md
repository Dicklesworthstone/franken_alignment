# Live actor-to-supervisor publication

This Linux reference integration replaces the operator's submit-file handoff with
an authenticated actor socket. The actor supplies the original Submit command and
can poll or cancel it while helpers and the independent human reviewer run. All
admission, review, approval, publication and outcome decisions still come from the
original owners. No new executor, dependency, wire protocol or authority journal
is introduced.

## Commands

```text
supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE
supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE
supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE
supervise_publication actor-submit ACTOR_PROFILE ORIGINAL_SUBMIT_JSON
```

The supervisor commands read the private Config. The actor command does not: it
reads only its connection profile and its own original Submit document. Reviewer
and stop commands continue to use the separate reviewer profile. Never provision
the private Config, journal directory, producer data or reviewer credentials to
the actor as a substitute for this role split.

One service invocation handles **one independently selected request ID**. This is
not a multi-request daemon. A new accepted request passes through fresh native
file-source admission, then the same `execute_serviced` workflow as file-based
commands. Checked modes retain their original structured or whole-input recipe,
coherent producer acquisition, feed freshness and two-key publication checks.

An existing store is opened through its original recovery fence, never created on
absence. A recorded request is reconciliation/observation-only: it cannot recreate
a helper job or regain an old automatic or human key. A new request must carry the
correct post-recovery policy epoch and current target version. The service never
rewrites actor fields to satisfy them.

## Independently supplied actor profile

```json
{
  "schema": "fa.actor-service/1",
  "clock": "unix_milliseconds",
  "request": 7,
  "scope": {"tenant": 1, "principal": 2, "run": 3, "branch": 4, "authority": 5},
  "socket": "/srv/fa-actors/actor-7.sock",
  "supervisor": {"uid": 1000, "gid": 1000, "pid": null},
  "actor": {"uid": 1001, "gid": 1000, "pid": null},
  "candidate_limit": 16,
  "connection_limit": 4,
  "exchange_limit": 1024,
  "runtime_ms": 15000,
  "poll_ms": 10,
  "reply_ms": 2000
}
```

These identity numbers are illustrative, not deployment defaults. Both UID and
GID must match. Explicit null PID admits all connecting processes with those
credentials; a positive PID narrows the policy. The connected server is verified
before the actor client sends any bytes. The supervisor verifies the connected
actor before source acquisition or protocol processing. Neither side accepts an
identity supplied by the peer's message.

The socket's parent must exist, be owned by the declared supervisor UID/GID, and
not be group/other writable. The new socket is mode 0660. Group traversal and
membership for separate UIDs are operator responsibilities. Existing paths are
never removed to make bind succeed; cleanup checks the created socket's inode and
device. Actor/reviewer/stop paths must be distinct. Profile parsing rejects unknown
fields, missing explicit PID policy, non-normalized paths, zero identifiers where
required, and out-of-range timing/connection/exchange limits.

## Review-time I/O, cancellation and uncertainty

Only the selected request can enter new admission. During work, the same socket
serves polls, cancellations and exact recorded retries through the original actor
wire. No new admission, evidence read or clock sample can occur through that
observation-only method, even if an unused snapshot exists. Independent operator
stop is serviced first at each workflow checkpoint. Actor cancellation cannot
release the charge of an already dispatched or unknown effect.

Socket loss alone is not cancellation or nonexecution. An accepted job continues
under the original deadline and controls; reconnects authenticate again and retain
the same ticket session and lifetime connection cap. All accepted/rejected
connection candidates count toward one fixed service cap. No rejection refreshes
that cap. Source failures remain redacted on the actor wire.

The actor CLI sends Submit once, then polls Pending and transient OutcomeUnknown
responses using the original exchange parser and one lifetime I/O budget. It never
reconnects or resubmits automatically. Timeout or transport failure records that
request bytes may have reached the server; only the original exact Submit document
may be used for a later explicit retry. Unknown is never printed as known success.
A received response followed by stdout failure also cannot trigger a resend.

After work completes, a finite read-free grace interval serves final polls and
same-session reconnects. Choose an exchange budget and polling interval sufficient
for the desired review duration, and a reply interval longer than the client's
polling interval. Service process success indicates completion of service work,
not that the proposed effect was approved; the actor's native Knowledge response
is authoritative for its observed request result.

## Boundaries and verification

This is cooperative one-request orchestration, not hard real-time preemption or
an admitted production runtime. Blocking filesystem work is still synchronous.
SO_PEERCRED identifies the connecting process credentials, not executable code,
a human, a remote identity, or the current holder of a transferred descriptor.
Filesystem isolation, secret placement and group membership must be independently
enforced. A process with private journal access is not contained by this socket.

The tests pair allowed execution with cancellation, source-free recorded retries,
wrong actor and server credentials, no-clobber/scope failures, strict profile
parsing, late whole-input changes versus quiet observations, and failed client
output. Synthetic helper verdicts exercise enforcement, not detector quality.
The Rust compiler, tests, formatter and Clippy have not run in the editing
environment. Required RCH commands fail to start because `rch` is absent.
