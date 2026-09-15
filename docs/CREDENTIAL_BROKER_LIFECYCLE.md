# Broker credential rotation and terminal revocation

Consumer: the trusted enforcement owner of the narrow brokered file-publication route.
This extends the credential broker without adding actor authority, a new effect command,
or a second publication state machine.

## Rotation is not authorization

`rotate_credential` replaces only the broker-held provider secret. It is bound to an
operation ID, the current credential generation, and exactly the next generation.
An exact retry with the same request and secret returns the original receipt; reusing
an operation ID with different material refuses. Stale generations and skipped
generations refuse. At most 64 rotation/revocation transitions are retained.

Rotation does not touch the action ledger, control sequence, policy epoch, congress
review, human approval, dispatch envelope, endpoint version, or resource accounting.
An already authorized original `DispatchEnvelope` may still be delivered by the trusted
broker after rotation, but only through the same endpoint checks and idempotency rules.
The new secret therefore changes provider access, not what effects are authorized.

## Revocation stops credentialed delivery, not reconciliation

`revoke_credential` is terminal for this broker owner. After it commits, `deliver`
refuses before the provider credential is presented. The operation itself does not
claim that an already sent or externally executed effect vanished and does not refund
an outstanding charge.

Clock observation, dispatcher fencing, endpoint status, sealing, and expiry resolution
remain available after revocation. Those operations do not present the effect
credential. A delayed effect that never executed can therefore still obtain the
original endpoint nonexecution receipt and release its charge; a historical execution
receipt still wins and remains charged.

Rotation and revocation share one operation-key namespace, so a rotation receipt cannot
be recovered as a revocation receipt or vice versa. Exact revocation retries return the
original receipt without another transition. Once revoked, later rotation is forbidden.

## Recovery boundary

Credential generation, terminal revocation state, retained change receipts and the
presentation counter move with `RecoverableCredentialBroker` into its offline owner and
back through `reopen`. The raw credential and raw endpoint remain private. Reopening
still requires the original process-local endpoint identity and fresh elapsed-clock
observation before status or delivery.

This is process-local control state. Losing the whole controller process still loses the
credential lifecycle state; it is not written into the endpoint publication file. A
future durable credential vault would need its own authenticated/anti-rollback contract
and must not reconstruct action authority from stored secrets.

## Verification boundary

Four module tests cover exact rotation retry, conflicting/stale rotation, terminal
revocation with nonexecution reconciliation, and persistence of rotation/revocation
through the existing offline endpoint-recovery handoff. The earlier broker integration
and recovery suites continue to cover real congress authorization, file publication,
lost acknowledgments and endpoint identity binding.

The tests have not been compiled or executed in this editing environment. RCH remains
unavailable, so formatting, Clippy, tests and doctests are also pending. Source
publication does not close FA-011/FA-012, a Beads task, or a production gate. Provider
credential authentication, remote credential APIs, durable secret storage, independent
watchdogs and hostile-host containment remain outside this bounded reference profile.
