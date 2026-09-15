# Broker-held credentials for one disposable file publication route

Consumer: a trusted enforcement host that already owns the original `OversightBroker`
and its Unix `PublicationEndpoint`. This closes one concrete part of the G1 perimeter
path (FA-011/FA-012): a declared `brokered_effects` file-write route can be bound to
a broker-owned credential and the existing receipt-gated publication endpoint.

## Narrow contract

`CredentialBroker` accepts only a `DispatchEnvelope` already produced by the
original action authority. There is no raw payload, target, shell, network, or
actor-command entry point. The broker owns both the credential and endpoint; it
exposes neither. The endpoint must already be the repository's Unix file-publication
profile and must already have been attached to the original authority domain.

Attachment rechecks the loaded perimeter route. The declaration must be
`brokered_effects`, its bypass must be `blocked`, its threat must be
`direct_credential_or_egress`, its actor credential disposition must be
`broker_mediated`, and its trust path must include the enforcement domain. This
reference adapter accepts only effect-purpose scopes and maps them to perimeter
purpose `1`. The declared effect must be `file_write`; the profile ID is fixed to
`fa.disposable-file-publication`, and the profile generation is bound to the
endpoint target's contract version.

The current inventory schema declares broker credentials family-wide rather than
naming the exact credential used by each route. Accordingly the secret supplied at
bootstrap is a trusted enforcement-host input. This increment does not claim to
authenticate that secret against an external provider. It does ensure that a route
with an actor-direct credential or nonbrokered mediation cannot instantiate this
adapter.

## Credential use and recovery

Before presenting the secret, `deliver` checks exact action scope plus the target's
adapter/object/contract/generation identity and rechecks the immutable route
classification. The original endpoint then checks its process-local dispatch
binding, dispatcher epoch, version, deadline, payload/units, retention, and exact
receipt history. A foreign envelope with matching numeric target fields therefore
still refuses.

Credential presentation is counted before the endpoint operation and remains
counted on refusal or lost acknowledgment. Repeating the *same* authorized envelope
is allowed and remains endpoint-idempotent: one external execution, one terminal
receipt, potentially multiple credential presentations. The broker never mints a
replacement permit or changes the action.

Status reads, sealing, expiry resolution, clock observation and dispatcher fencing
remain control/reconciliation operations on the hidden endpoint. They do not
present the effect credential. Thus a missing dispatched effect can be sealed and
refunded only through the original endpoint receipt without needing another effect
credential presentation. An already executed effect remains charged.

The broker does not expose the endpoint for process-loss recovery in this initial
profile. The existing `FileEndpointRecovery` key may still be retained by the
trusted controller, but a later recovery integration must preserve the broker role
rather than returning a raw endpoint to an actor. This source does not claim that
boundary yet.

## Verification boundary

Seven integration tests use the original full-input congress, original action
permit, an actual Unix file publication, and the new broker. They cover successful
publication, lost acknowledgment with exact idempotent retry, nonexecution sealing,
foreign-envelope rejection, wrong mediation/effect/profile/scope, refusal of a
memory-only endpoint and experiment scope, and credential size/redaction.

The module also contains compile-fail examples preventing direct access to its
private adapter and credential bytes. Helper verdicts remain deterministic test
fixtures, and the perimeter inventory and supplied credential remain operator
assumptions.

Rust compilation, formatting, Clippy, tests and doctests have not run in this
editing environment. RCH remains unavailable. No Beads status or production gate
is promoted by source publication. This does not yet implement authenticated
credential rotation, a remote provider protocol, whole-process broker recovery,
anti-rollback storage, or a complete deployment perimeter.
