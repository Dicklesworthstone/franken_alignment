# Durable credential lifecycle in FileOversight

The durable credential guard now has a nonsecret generation and terminal revocation state. Rotation invalidates every previously bound process-local credential permit without mutating policy, review, action authority, rights accounting, or already charged effects. A fresh independently supplied broker/provider pair may be bound at the new generation.

`rotate_credential_guard` journals only operation ID, expected generation, and the next generation. `revoke_credential_guard` journals only operation ID and expected generation. Secret bytes and secret hashes never enter the authority journal. Operation IDs are retained so exact retries return the original receipt without another event; conflicting reuse or stale generations refuse.

Revocation is terminal for new credentialed execution, but status, sealing, deadline resolution, and reconciliation remain available. This prevents credential loss from turning an already charged effect into an accounting dead end. Historical endpoint receipts remain authoritative.

Generation, revocation, and operation receipts replay across `FileOversight::open`. Reopen also creates a fresh process-local issuer, independently invalidating every pre-recovery permit. A non-revoked recovered owner therefore requires a newly supplied live pair before execution.

The same atomic canonical replacement and ambiguity rules apply as elsewhere in the durable authority. Faults around Stage, Write, FileSync, Rename, and DirectorySync expose no candidate state through the faulted owner. Reopen determines whether the lifecycle transition is actually present in the canonical file, and exact retry follows that recovered cut.

Credential rotation and revocation remain available during a registered evidence-source interruption because they create no source evidence and grant no action authority.

The learned executable exposes these same transitions through
[`--rotate-credential` and `--revoke-credential`](LEARNED_CREDENTIAL_LIFECYCLE.md).
The offline adapter requires the complete guarded learned recipe, an independent
input history anchor and a new output archive. It checks the original operation
before recovery changes the journal. Each successful invocation still performs
the original recovery Fence, including exact lifecycle retries; the receipt's
historical generation is reported separately from the current credential epoch.

Regression source covers old-permit invalidation, exact and conflicting retries, stale generations, terminal revocation with credentialless endpoint sealing/refund, complete authority reopen, and five storage barriers for both rotation and revocation.

This remains trusted operator configuration and ordinary process-memory secret
handling, not a cryptographic credential vault. Journal authenticity and
anti-rollback storage remain separate requirements. The original source handoff
recorded compilation, formatting, Clippy, tests and doctests as unexecuted because
its required RCH runner was unavailable. Later scoped native and operator
execution evidence is recorded in
[implementation status](../IMPLEMENTATION_STATUS.md); it does not establish full
workspace or production-gate qualification.
