# Independent broker/provider credential bootstrap

The concrete brokered file-publication profile now receives two separately
constructed capabilities: `BrokerCredential`, retained by the enforcement-side
broker, and `ProviderCredential`, representing the independently configured
provider-side expected material. Neither exposes its bytes, implements `Clone`,
or reaches the actor-facing API.

This closes a defect in the first reference broker: the adapter expectation was
created by cloning the broker secret, making the credential comparison
self-fulfilling. `CredentialBroker::new` now refuses unless the independently
supplied values agree before either capability or endpoint is retained. A
mismatch cannot execute, change the endpoint, increment credential-use accounting,
or alter action authority.

Credential rotation follows the same rule. `rotate_credential` receives a fresh
broker capability and a fresh provider capability. They must agree before any
mutation. A mismatch leaves the existing generation, broker material and provider
expectation untouched. Exact retries bind the same operation, generation and new
secret bytes. Rotation still does not create a proposal, review, permit,
dispatch envelope, receipt, refund or wider policy.

The provider expectation remains part of the broker-owned adapter state across
the existing offline/reopen handoff. Reopening therefore cannot reconstruct the
provider side from the broker credential. Terminal revocation retains its prior
semantics: credentialed delivery stops while status, sealing, expiry resolution,
fencing and clock observation remain available for outstanding-effect
reconciliation.

This remains a reference model. Both capabilities are trusted operator inputs and
are held in process memory; equality is not a cryptographic authentication
protocol or constant-time secret comparison. The file adapter is not a real
remote provider, no encrypted credential vault is implemented, and whole-process
loss of credential lifecycle state remains outside this increment. The current
perimeter schema also declares broker credentials family-wide rather than binding
each route to a specific named credential; that is a separate schema boundary.

New source coverage includes bootstrap mismatch, successful independently supplied
agreement, mismatch-safe rotation, idempotent matching rotation, file publication,
endpoint recovery, actor-supervisor execution and redacted/bounded capabilities.
The required RCH runner is unavailable in this environment, so Rust compilation,
formatting, Clippy, tests and doctests remain unexecuted. No Beads or production
gate status is changed by this source work.
