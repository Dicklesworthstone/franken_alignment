# Durable publication through the Tower

On Unix, `fa_reference::tower::publication::PublicationTower` connects the
registered `propose`, `get` and `precheck` operations to an original
`FileActorPort<FileOversight>`. It uses the same `TowerRequest`, verb vocabulary
and sixteen-field response envelope as the [snapshot Tower](TOWER_REFERENCE.md).
The original file owner supplies durable intake, exact proposal binding,
request-local observations and recovery. This adapter owns no authority journal.

## Mount one actor session

The host creates or opens its original `FileOversight`, obtains its actor port
and supervisor, and mounts that port with `PublicationTower::new`. The mount is
an explicitly versioned `Run` address. Its tenant and run are the canonical
decimal values of the original native scope. A branch address or a different
tenant/run refuses installation. The generation identifies the host's route
registration; it is not a policy epoch, request generation or control sequence.

Each Tower instance belongs to one host-authenticated actor session. The host
must provision `TowerAccess` and must not accept grants from actor request bytes
or share the session's tickets with another principal. This Rust adapter checks
those grants; it does not implement transport authentication.

The supervisor remains outside the Tower. Only it supplies a fresh one-use
admission snapshot, performs original helper commitment/reveal review, obtains
the original automatic and human keys, and invokes dispatch and publication.
The Tower retains a weak actor port, so dropping the supervisor releases the
original authority lock and makes further observations explicitly unavailable.

## Inspect exact policy before proposing

Precheck defaults to unavailable. The supervisor explicitly issues an original
`FileActorPrecheckPort` and retains its `FilePrecheckDisclosure` custodian. The
host installs that port with `set_precheck(Some(port))`. The adapter checks the
original owner allocation, not only numeric scope equality. A port from another
owner refuses even when its tenant, run and complete native Scope are identical.
Every replacement first withdraws the old installation, including an invalid
replacement. `set_precheck(None)` withdraws this facade's copy without revoking
other copies or changing the native grant's lifetime.

Use `Verb::Precheck` at the mounted run with
`ReadArguments::Precheck { query, proposal }`. The query label is nonzero; the
proposal retains the same exact fields used by real submission. The mounted
generation may be omitted for this read or must match exactly when supplied.
Subject scope is required, but an operator role, complete-domain grants and a
Permit purpose are not prerequisites for using an installed disclosure.

Malformed or unauthorized Tower requests refuse before entering the native
query allowance. Once entered, the original port checks current disclosure
custody, owner availability, exact revision and original source eligibility.
It evaluates only disclosed exact constraints: target/ceiling, deadline,
available resources and the actual retained policy predicates. Native denial
remains a Known precheck result, with common reason `DeniedByPolicy`; a pass
retains `precheck: true` and `review_required: true`.

The adapter has no precheck outcome cache. A supervisor-provisioned native
witness cache remains effective, including its original source invalidation.
Every entered query still consumes the same finite native allowance, even a
cache hit, repeated query label or new facade using a clone. An optional envelope
idempotency key neither deduplicates a precheck nor acquires a request ticket.

Precheck does not supply an admission snapshot, reserve rights, append a journal
event or call an endpoint. Actual submission still requires its original fresh
snapshot and review. A subsequent owner transition makes the old disclosure
ineligible; it cannot reuse an earlier pass. The host may install a newly issued
same-owner disclosure without discarding the session's existing request tickets.

## Submit an actual proposal

Use the mounted run address with its exact registration generation, `Verb::Propose`,
and `ReadArguments::Propose { request, proposal }`. The nonzero native request
identity must match `idempotency_key` as a canonical decimal string. Arbitrary
string keys are not hashed into native identities. The proposal contains the
exact target, payload, units, deadline and expected policy epoch.

The caller needs the subject's tenant/plane/authority grants and an explicitly
allowed `Permit` purpose. This is permission to use the already provisioned
actor intake; it is not a `PermitCx`, effect permit or supervisory role.
Operator status and global-domain visibility are not required for an actor to
submit its own proposal. Original native checks still decide whether admission
is possible under the supplied policy epoch and current one-use snapshot.

The adapter transfers the owned proposal directly to a validated typed
`ActorWire::execute` path. Byte parsing, byte encoding and typed execution share
the same command validation. The existing bounded session ticket map and native
durable owner remain the only request owners. No payload-to-hex-to-JSON round
trip, duplicate proposal ledger or cached outcome is introduced.

A successful submission can return native `Pending` or `NotAdmitted`; it does
not report a completed effect. The normal supervisor workflow can subsequently
produce `Denied`, `CancelledBeforeDispatch`, `Executed`,
`ConfirmedNotExecuted` or `OutcomeUnknown` through the original owner.

## Observe and recover the original request

`request_address(request)` returns a namespaced `Attempt` address for the actor's
intake-request projection, including the original run/principal/branch/authority.
It is deliberately distinct from a native ledger attempt number. `get` at that
address polls a ticket acquired by this session. Knowing or guessing a numeric
request identity does not acquire another session's ticket.

Each admitted `get` and each exact `propose` retry polls the original owner.
Pending results can therefore change to explicit uncertainty or a terminal
outcome. The adapter never caches a response that would conceal that change.

Reusing the same native key with the same complete proposal reacquires its
original ticket without another durable request or execution. Changing its
proposal conflicts even if the new proposal would otherwise be admissible.
`operation_id` is envelope correlation metadata, not a second effect identity.

After the owner is reopened through its original recovery path, a fresh Tower
session has no old tickets. An exact submit retry reacquires only the original
request's observation. It does not require a fresh admission snapshot for an
already retained request, resurrect old publication keys or resubmit an effect.
Native recovery and liability accounting remain authoritative.

## Preserve the native epistemic boundary

`PublicationResponse` specializes the common `TowerResponse` with
`PublicationResult` and `PublicationLimits`. Its structured `publication-request`
body contains the original actor `WireResponse`, including every native
`Knowledge` state and the original request-local `ActorBasis`. Common admission
refusals use the shared `Knowledge` encoder. There is no synthetic outer basis
claiming that a native request label is a control cut.

The `precheck` body embeds the original `PrecheckWireResponse`. Its disclosure
ID, query label, policy epoch and observed elapsed tick remain in the original
`FilePrecheckBasis`; they never become a global generation or control sequence.
Revoked disclosure stays Withheld, unavailable/stale source eligibility stays
Unknown, and a mismatched policy epoch stays a native Stale observation.

The envelope keeps all sixteen common fields. Global generation, control
sequence, source frontiers, budgets, realized cost and uncomputed affordances
remain `Unknown`, or `Withheld` on an unauthorized request. Neither a native
request generation nor a mount generation is substituted into those fields.
The address retains the explicit mount generation; a stale route points to the
current mounted address in `would_change_this`.

Native errors remain present in the structured body while the common envelope
maps them to registered reason codes. An unavailable controller is distinct
from an unknown external outcome. Exact keyed proposals are retry-safe in the
native idempotency sense; an unknown outcome is never converted to a refund or
permission to create a new key.

## Bounds and remaining scope

The native session retains at most 128 tickets. Each native precheck disclosure
allows at most 128 queries, potentially fewer when issued by the supervisor;
its remaining allowance is not disclosed by this adapter. Native proposals retain their
existing target, payload, unit and deadline checks, and the durable owner retains
its own request/byte limits. Envelope operation IDs and keys are bounded to
128 bytes and addresses to 256 bytes. Responses have a configurable 16 KiB to
1 MiB bound, with 16 KiB by default. A post-mutation encoding refusal preserves
the exact retry key and never implies that durable admission was rolled back.

This is a concrete ordinary-publication profile. It does not expose supervisor
mutations, arbitrary affordance execution, a planner's decision card, learned
generation, a CLI, network authentication, branch rehearsal or independent
receipt verification. Unsupported verbs refuse before touching the native owner.
The complete dispatcher and deployment qualification contracts remain open.

Current qualification is recorded in the
[publication receipt](../artifacts/execution/2026-11-06-tower-publication-receipt.json)
and subsequent [precheck receipt](../artifacts/execution/2026-11-06-tower-precheck-receipt.json).
Publication's library and selected test targets passed type checking before the
precheck extension. Runtime execution and the final extension's compiler check
remain blocked by the recorded shared-memory limit; those earlier checks do not
qualify the later source.
