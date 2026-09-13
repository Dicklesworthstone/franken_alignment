# Durable worker sockets for the full-input, two-key publication host

## Implemented consumer

`FileOversight::begin_helper_review` returns a `FileHelperPool` that drives actual
preprovisioned Unix streams into the existing durable observed-review methods.
This supplies the missing worker-transport connection to `FileOversight`, not a
second congress, alternative runtime or bypass around human approval. It serves
the reference portions of FA-018/019 and FA-014 at the existing L3/L4/L5 boundaries.
The full native/foundation admission and host qualification remain separate.

The implementation reuses the original `HelperPort`, `HelperConnection`, exact
worker wire format and worker-side `HelperClient`. The existing in-memory
`HelperRound` and the new durable pool share one private `Coordinator` extracted
from the original worker-slot implementation. Native public signatures are
unchanged. The coordinator forwards events to original sessions; it cannot
compute a vote tally, construct a completed review or authorize an effect.

## Launch and exact inputs

A `FileHelperLaunch` names the attempt, new round, evidence root, logical cutoffs,
expected input revision, complete socket roster and original helper limits.
The input must already be recorded in the original `OversightBroker`. The full
roster, logical input/salt limits, nonblocking setup and each encoded request are
checked before beginning the durable round. Launch sends no request bytes.

Each socket receives ONLY its assigned original EvidenceViewManifest's actual
input, profile and ordered parts through the unchanged worker format. Peer
contexts and local policy witnesses are not automatically forwarded. The operator
must authenticate and isolate sockets before provisioning them. The unchanged
reference digest is not a cryptographic commitment and does not attest a model.

Once launch begins its native session, that live round is leased to its returned
pool. Public manual commit/open/reveal/finish methods refuse that round, including
after the pool is dropped. There is no reduced-roster, replacement-stream or
caller-vote fallback. The lease is process-local custody: recovery already
invalidates every native session and retains all started round IDs, so reopening
cannot revive a forgotten lease. A new explicit round remains possible when the
original controller allows it; the pool never retries or reauthorizes by itself.

## Durable acceptance before phase progression

Every pool pass makes one original nonblocking I/O step per member, observing the
host's logical clock before and after each step. A commitment received at its
cutoff cannot be backdated with a helper-supplied timestamp. Every changed clock
observation is journaled; repeated observations of the identical logical tick do
not append redundant clock events or extend any deadline.

Accepted commitments, reveal opening and accepted reveals each execute through
`FileOversight`'s original journal transaction BEFORE their worker slot advances.
In particular, a reveal request cannot be sent based on an unacknowledged
commitment or uncommitted reveal-opening event. Protocol rejections remain explicit
per-member failures. Storage or resource exhaustion is an outer failure: it closes
all connections rather than treating the failed transaction as a missing vote.

`FileHelperFailure` retains the actual I/O reports already produced during that
pass, including the step preceding a post-I/O persistence failure. Successful
previous commits are not rolled back, and canonical storage may be newer after
an unacknowledged rename. The unavailable host cannot return candidate decisions,
keys, receipts or refunds. Retained worker status is not fresh external evidence.

## Finish and effect authority

`finish` performs no socket I/O, implicit clock advance, provider read or helper
rerun. The supplied tick, current full input and policy snapshot go through the
original completed-review application. A premature finish preserves the pool.
Missing or failed workers remain in the native roster until its original cutoff.
A completed stale-input refusal is committed and consumes the round; the caller
cannot repair it and reuse the completed judgment. Restrictive reviews retain
the original `current=None` behavior.

A successful review is still not a permit. The original automatic reservation,
separately held human key, full-input binding, policy/control epochs and endpoint
expiry remain mandatory. Stop/recovery clears native sessions; the next pool
operation then closes rather than progressing an old helper protocol. Dropping
sockets does not cancel an action, refund an uncertain effect, or prove absence.
The original endpoint receipts independently settle existing obligations.

## Bounds and verification

Original member/input/salt, frame-size, one-reply-slot and I/O-step bounds remain
unchanged. Private port/input copies coexist with retained journal projections.
Socket work bounds do NOT bound blocking filesystem latency, replay CPU, total
RSS, allocation traffic or external-model inference. Changed ticks and accepted
phases consume the existing 4,096-event/16-MiB journal allowance, with no new
capacity minted after failure or restart. The underlying bounded full-history
replay/rewrite remains quadratic over a complete operation sequence.

Nine new public test functions use real Unix socket pairs and the unchanged
worker client. They cover successful full-input/two-key publication followed by
owner recovery; exact isolated member inputs; missing-helper cutoffs; stale-input
refusal followed by a genuinely new round; no manual-vote fallback; all-roster
and input-budget admission; a cutoff crossed specifically after read; foreign
owners and recovery; and terminal stop alongside an already-published unknown
effect. One compile-fail example checks that a worker pool cannot authorize.
Existing worker tests and original authority/endpoint assertions are unchanged.

The command below was attempted and failed before compilation with
`rch: command not found` (exit 127). No local build fallback was used. All new Rust,
socket scenarios, formatting, Clippy and revision-bound qualification remain
UNEXECUTED. No Beads item or production gate is closed by this source construction.

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_helper_transport
```

No model inference or authenticated peer evaluation ran during editing. Operator
clock correctness, plaintext storage, cooperative locking, hostile-path exclusions
and missing independent anti-rollback/authentication from FILE_OVERSIGHT_RECOVERY
remain unchanged. This is a synchronous reference integration, not a qualified
production executor, secure sandbox, or full durable serving-host implementation.
