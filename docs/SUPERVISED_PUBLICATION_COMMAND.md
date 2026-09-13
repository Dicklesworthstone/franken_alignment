# Runnable supervised publication and independent terminal review

## What this command does

`examples/supervise_publication.rs` is a synchronous Unix consumer of the existing
reference implementation. An original actor Submit document passes through the
original actor gateway, durable full-input supervisor, actual configured helper
executables, independent reviewer socket, two keys, guarded publication and
endpoint reconciliation. It contains no alternative policy evaluator, vote reducer,
receipt mechanism, sampler or authority ledger. It does not launch the actor itself.

The canonical publication is still the payload in the original protected journal,
read through `FileOversight::read_publication`. The command does not perform an
additional unmediated copy to an arbitrary destination, upload a package or send a
network request on the actor's behalf. `inspect` exposes that historical payload as
hex for the operator. This is a working source-level end-to-end reference consumer,
not a qualified production deployment or a claim that compilation has succeeded.

## Operator configuration

The complete schema example is `fixtures/supervised_publication.json`. All fields
are required, unknown and duplicate keys refuse, and native constructors validate
the complete original policy, congress, helper contracts, human profile and source
configuration before the command creates a store or starts a program. Policies
support the original target/payload/unit predicates, exact/absent/range reads,
All/Any and Not nodes; their original topological/reachability rules still apply.

`store` must name a NEW protected directory for `create`; its ancestors and the
configuration, evidence producer, executables and working directories must be
operator controlled. Helpers supply their own executable, explicit arguments and
explicit environment. The existing launcher clears ambient environment and uses
private inherited stdin sockets; each program must implement the existing
`HelperClient` protocol. There is no shell, model download or default helper that
returns Allow. The fixture's `/operator/...` paths describe deployment choices and
must be replaced with actual configured programs and paths.

Actor metadata is supplied explicitly and accepted only with `audit_only` grade.
It is not fabricated tensor state or a claim of numerical checkpoint continuation.
The command enables the native file-source gate, first-publication guard and
`RecoveryReserve::terminal()` before work. One target and its initial expected
version come from the independent bootstrap. Configured labels/cohorts do not
establish statistical helper independence.

The clock profile is explicit: Unix-epoch milliseconds, domain
`0x4641554e49584d53` (ASCII `FAUNIXMS`). A restarted process never substitutes its new
`Instant` origin for saved authority time. A backward wall-clock observation is
rejected by the original ledger. An unavailable/overflowing wall clock aborts
rather than supplying a default time. Clock accuracy and continuity remain host
assumptions. A separate monotonic timer only bounds command waiting and requests
shutdown; it does not mint an elapsed-time observation or undo effects.

## Invocation

Build or run through the repository's required RCH/operator workflow. The example
binary has these modes:

```text
supervise_publication proposal CONFIG REQUEST_ID PAYLOAD_FILE TTL_MS
supervise_publication create CONFIG SUBMIT_JSON
supervise_publication review CONFIG REQUEST_ID
supervise_publication resume CONFIG SUBMIT_JSON
supervise_publication inspect CONFIG
```

`proposal` emits the original actor-wire JSON, with an absolute millisecond
deadline, exact payload bytes and the initial epoch/target from the configuration.
It grants nothing. Save those exact bytes for recovery. This convenience mode is
for a new deployment, not a way to rebase an old action to a new epoch/version.

`create` admits exactly that explicit request and starts the configured helper
roster once. Request IDs also name this command's single round and human request
in their distinct original namespaces. If the congress does not permit continuing,
unused work is cancelled through the original ledger; no weaker roster or second
round is substituted. Otherwise it announces a private endpoint on operator stderr:
`STORE/review-REQUEST_ID.sock`.

In a separate operator terminal, run `review` with the independently supplied
configuration and selected request ID. It checks the configured reviewer, scope,
clock domain and selected request before displaying the complete original offer.
Payload and actual input bytes are reversibly escaped, including controls,
non-ASCII bytes and backslashes. The full original manifests, ordered parts,
profiles, omissions and provenance are also shown. Nothing is summarized or
silently truncated. A failed display or flush cannot enable a decision.

The terminal requires an explicit `APPROVE`, `REJECT` or `REVOKE` line containing
both the original request ID and this offer's complete session nonce. Blank input,
EOF, a wrong offer, an oversized line or `yes` sends no decision. The command
requires interactive input/output terminals and has no `--approve` option. The
original client still enforces legal decisions for the offered disposition.
Human typing itself is blocking; the host independently enforces its action and
workflow deadlines. A response typed after expiry is not sent.

Only the original durable reviewer operation can produce the host's second key.
A receipt lost after that operation commits is reported as receipt loss, not a
rejection or permission to regenerate the key. The following dispatch rereads the
source on both sides of authorization; first publication reads again. Changed
context, stale policy, expiry and unavailable evidence still fail the original
checks. Select a source lease suitable for the human review interval: an expired
lease is refused, never silently widened by the console.

The final actor response on stdout is encoded by the unchanged `ActorWire` and
contains only its redacted Knowledge result. Create/resume exit successfully only
for an original Known(Executed) outcome and confirmed direct-child cleanup. Held,
denied and unknown outcomes remain distinct in that JSON; nonzero exit is not a
nonexecution receipt. Operator diagnostics are not substitutes for the result. A failed output write cannot reverse committed
work. On a workflow error the command attempts the original reserved-space terminal
stop and separately reports drain failure. Cleanup retains and polls direct-child
owners; exceeding the cleanup waiting budget reports unconfirmed reaping rather
than claiming complete shutdown.

## Recovery

`resume` opens the same store with the independent exact profile, performs the
original recovery fence, and requires an existing request matching the ENTIRE
saved Submit document. It cannot create a missing request, reread the evidence
file, launch helpers, obtain a new human key, rebuild a publication job or resend
an effect. Only the original reconciliation path may resolve an uncertain outcome.
A retained execution stays executed and charged; missing terminal evidence stays
unknown and charged. Cancelled work is not automatically re-proposed. Repeated
opens still consume the original finite journal recovery capacity.

The review listener never deletes an existing path to make startup succeed. Normal
cleanup removes only the socket inode created by that invocation, under the trusted
namespace assumption. A stale socket after abrupt termination requires explicit
operator handling; query-only `resume` does not need a reviewer socket.

## Boundaries and source verification

This is not an OS sandbox, authenticated human service, TLS endpoint, hostile-host
anti-rollback system or a trained-helper implementation. In particular, private
paths do not isolate hostile programs running with the same operating-system
identity. Helpers must be trusted executable wrappers or externally isolated, and
the reviewer channel must be protected by the operator. Model output remains data
in the original bounded helper protocol. No claim of complete mediation follows
from this command alone.

Five scenario tests and one inert helper-process entry are included with the
example. They use real inherited sockets and child executables, an explicitly
synthetic independent reviewer client, exact publication/rejection controls,
source-free exact recovery, changed-request refusal, strict configuration negatives,
terminal escaping and failed-display/no-default-decision checks. They are not
trained model or human-authentication evidence.

The attempted command was:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --example supervise_publication
```

It failed before compilation with `rch: command not found` (exit 127). The new Rust,
subprocess scenarios, formatting and Clippy have not been executed in this editing
environment. No Beads task, production gate or earlier execution claim is changed.
