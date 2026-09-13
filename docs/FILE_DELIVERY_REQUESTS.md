# Durable request identity at the original file-publication boundary

Consumer: an actor-facing adapter whose external submission key must survive
complete loss of its process, not merely a reconnect to a surviving ActorPort.
This extends the existing FileDelivery reference profile (plan 8, 16, 17; FA-107)
without changing its original policy, congress, rights or endpoint reducers.

submit_request persists the original exact ActionSpec, trusted policy snapshot
and external key as journal input. Replay invokes the original proposal method
and recomputes the admission; no accepted outcome is deserialized as authority.
Keys do not select ledger IDs. Allocation avoids existing operator attempts and
retains refused allocations rather than recycling their identity. Structural or
journal admission failures create no request; an admitted policy/resource refusal
is retained as NotAdmitted and cannot be retried into acceptance under that key.

Exact retries compare every original action field and return the current durable
request disposition without another event, clock observation, proposal or permit.
They work after epoch changes, stopping and capacity exhaustion. Changed payloads,
versions, targets, deadlines, bounds, scopes or policy epochs conflict. Trusted
snapshots are not part of the actor's idempotency input: replacing one cannot make
an exact retry rerun admission or rebase an old effect onto new evidence.

Publication and authority acknowledgment remain separate original transitions.
A lost acknowledgment stays unknown after reopening until endpoint reconciliation.
Recovery fences old permissions and cancels only undispatched reservations; retry
cannot recreate them. New work requires a new key and the current explicit epoch.
Request cancellation uses the original ledger before dispatch, and is a no-op for
sent/terminal work, never nonexecution evidence or a refund. Stop and reconciliation
project their results onto the retained request without another outcome ledger.

Request-local generation changes reflect only externally distinguishable pending,
unknown and terminal phases, not private congress activity. Status/action lookup is
supervisor data, not an actor capability. Ambiguous storage blocks even exact retry
and status reads until exclusive recovery; no speculative candidate is returned.

The fixed 128-record/2-MiB payload allowance includes refusals and terminal requests.
No eviction, cancellation or settlement resets it. Existing journal limits remain
independent and may bind earlier. New event tag 13 uses the existing bounded action
and snapshot encoding. Old event bytes and bootstrap profiles are unchanged; old
readers refuse the new tag rather than interpreting it as an original proposal.

Nine regression scenarios cover publication recovery, reservation nonresurrection,
refusal persistence, exact binding, cancellation, both retention limits, actual
staging-file failure and the distinct key/attempt namespaces. Rust compilation,
formatting, tests and RCH qualification have NOT run: the required RCH command
could not start because rch is unavailable. No original tests, dependency admission,
production qualifications or bead state changed. Journal authenticity, anti-rollback
storage, peer authentication and full OversightBroker/decoder migration remain
outside this narrow operator-controlled Unix file profile.
