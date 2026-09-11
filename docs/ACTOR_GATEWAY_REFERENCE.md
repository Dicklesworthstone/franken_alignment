# Actor proposal gateway

September 11, 2026: reference implementation of plan sections 17.1 and 9.3 (FA-107; FI-A15/FI-A16). The concrete consumer is an existing cooperative agent's tool dispatcher. It receives an `ActorPort`; a separately retained `ActorSupervisor` owns the existing `OversightBroker`. This adds no second authority ledger, runtime, storage backend or dependency.

## Intake and visibility

The actor submits only a target, exact payload, resource upper bound, deadline and expected policy epoch. The supervisor fixes tenant/principal/run/branch/purpose and derives policy witnesses through the existing proposal path. An actor cannot supply a scope, verdict, disqualifier, evidence root or permit. Intake processes FIFO arrival order, not caller-selected numeric priority.

Idempotency binds every proposal field. An exact retry returns the same ticket; changed bytes under that key refuse. Requests, original proposal bytes and terminal tombstones remain bounded and retained for the entire port lifetime. They are not evicted to make spent keys fresh. The limits are lifetime admission limits, not a claim of indefinite operation or restart-safe deduplication.

Actor polling is a `Knowledge` projection with request-local provenance. It emits Known only for terminal dispositions, Pending for review/reservation, Unknown for in-flight or unavailable outcomes, and a uniform Withheld for another port's ticket. The Stale and Absent variants are reserved data forms, not emitted by this complete in-memory request table. This is a reference projection, not implementation of the complete production FA-133 wire contract. No observation can be passed back as authority.

Neither shared state nor the actor handle's debug representation contains committee members, votes, questions, policies, exact failure diagnostics, private control sequences or effect permits. The supervisor receives those diagnostics through its own intake result and retains the privileged broker API. It must never be passed to the actor. Polling returns the last published projection; Pending never asserts that an effect cannot have started since that publication.

Cancellation is a request, not a nonexecution receipt. Queued requests may be stopped without creating an attempt. Accepted requests are cancelled only through the existing ledger and only before dispatch. Dispatched and unknown effects stay charged and visible as uncertain. A dropped supervisor leaves unfinished observations Unknown and stops new admission, while previously published terminal outcomes remain historical facts.

## Verification boundary

Public tests cover fixed scope, FIFO, exact retries, changed fields, epoch refusal, private diagnostic filtering, genuine congress approval remaining Pending, reservation cancellation, foreign tickets, terminal tombstones, exact/one-over byte capacity, malformed proposals and supervisor loss. A compile-fail example checks that the actor handle cannot become a broker.

The Rust sources and tests have not been compiled, executed, rustfmt-formatted or RCH-qualified in this environment; the required tools are absent. No historical receipt qualifies these additions and no production bead is closed. This is safe-Rust API role separation inside one process, not hostile-harness containment, authenticated identities, durable authority or a network/CLI service. Timing, queue capacity and eventual outcome remain declared feedback channels; no empirical leakage claim is made.
