# Actor execution through the credential broker

Consumer: the trusted `ActorSupervisor` for an actor whose effect route is the
concrete brokered file-publication profile.

`deliver_request_brokered` reuses the original actor request, complete helper review,
automatic permit and optional human key. It consumes that original authorization
through `dispatch_request`, then hands only the resulting immutable `DispatchEnvelope`
to `CredentialBroker`. The supervisor never receives the provider credential or raw
publication endpoint.

A successful endpoint receipt is accepted through the original authority before the
actor's outcome projection becomes terminal. Any broker/endpoint error after dispatch
is conservatively marked acknowledgment-lost; it never causes automatic re-proposal,
re-review, replacement authorization, credential rotation or effect resend.

`reconcile_brokered_pending` performs the same bounded outstanding-obligation sweep as
the raw-endpoint integration, but uses only broker status and expiry-resolution APIs.
It does not present the effect credential. Historical receipts win; a missing request
remains unknown until its deadline can be atomically resolved by the hidden endpoint.
Per-attempt failures remain explicit and do not roll back sibling progress.

`restart_brokered_dispatcher` advances the original dispatcher fence, installs that
fence through the hidden endpoint, and confirms the exact acknowledgment back into the
same authority. Reopening the endpoint through `RecoverableCredentialBroker` therefore
requires neither exposing the endpoint nor reconstructing permissions from actor state.

Credential revocation composes with this path: it can block a delayed credentialed
send after dispatch, while later endpoint-backed expiry reconciliation can establish
nonexecution and refund the original charge. Revocation is not itself nonexecution
evidence.

Four integration tests use the existing ActorPort/ActorSupervisor, actual helper
congress, original permit, real Unix file publication and concrete credential broker.
They cover successful actor publication, lost-ack reconciliation without another
credential presentation, terminal credential revocation followed by deadline-backed
refund, and dispatcher restart after endpoint reopen with the original execution
receipt retained.

The Rust has not been compiled or executed in this editing environment; RCH remains
unavailable. Formatting, Clippy, tests and doctests are pending. This source does not
close FA-011/FA-012 or any production gate, and it does not add peer authentication,
durable secret storage, a remote provider protocol or hostile-process containment.
