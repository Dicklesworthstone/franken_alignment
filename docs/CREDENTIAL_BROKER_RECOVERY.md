# Broker-role recovery without raw endpoint escape

Consumer: the trusted controller retaining the file publication recovery object for
the concrete brokered file-write profile documented in `CREDENTIAL_BROKER_PUBLICATION.md`.

`RecoverableCredentialBroker` owns the credential broker and the original
`FileEndpointRecovery` together. `into_offline` drops the live endpoint lock but
retains the route declaration, credential bytes, exact process-local endpoint
binding, credential-use counter and recovery key. The resulting
`OfflineCredentialBroker` has no deliver/status/seal API.

`reopen` uses only that retained key. The reopened endpoint must carry the same
process-local binding and must still satisfy the original perimeter route/profile
contract. A different file endpoint with identical numeric target fields therefore
cannot become the credential's destination. Current elapsed time is deliberately
not restored: the controller must observe a fresh clock value before delivery or
status queries, exactly as required by the underlying file endpoint.

An executed request with a lost acknowledgment can be recovered by querying its
original endpoint receipt, with no second credential presentation. A request that
never executed can be sealed and refunded through the original broker receipt,
also without presenting the effect credential. If the caller instead explicitly
re-delivers the same original authorized envelope after reopening, the credential
presentation counter advances but the endpoint's original idempotency still limits
external execution to once.

The broker never reconstructs an automatic permit, helper judgment, human key or
new dispatch envelope. It also never serializes the credential into the publication
file. Losing the entire process still loses both the credential and process-local
binding; this reference recovery only covers endpoint-worker interruption while
the controller-side broker owner survives.

Four regression scenarios cover executed lost-ack recovery, missing-effect sealing,
exact post-reopen re-delivery, and mismatched endpoint/recovery-key refusal. They
use the original congress, action authority, file endpoint, and receipt reconciler.
These are source scenarios only: Rust compilation, tests, formatting, Clippy and
doctests remain unexecuted because RCH is unavailable in this environment. No
Beads status or production gate is changed.
