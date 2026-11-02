# Durable credential guard for FileOversight

The durable full-input/two-key publication owner can now require an independently
supplied credential pair at its first externally visible execution boundary without
persisting either secret in the publication journal.

## Journaled contract, process-local secret capability

`enable_credential_guard` resolves the exact v1 perimeter scope/family/route,
requires the existing unambiguous broker-credential rule, and durably records only
`FileCredentialPolicy`: family, route, declared credential name and adapter-profile
generation. The inventory must declare `brokered_effects`, blocked bypass,
`direct_credential_or_egress`, `file_write`, broker-mediated actor credentials,
an enforcement trust hop and the `fa.file-oversight-publication` profile matching
the endpoint contract version.

The same transition enables the existing first-publication evidence guard. There
is therefore no legacy raw `publish` fallback after credential mediation is
installed. Installation is bootstrap-only and refuses once effect work exists.
Old journal tags are unchanged; the new contract and credentialed publication use
new bounded tags.

Secret material enters only through `bind_credential_pair`. It accepts the
independent `BrokerCredential` and `ProviderCredential` capabilities, revalidates
the supplied inventory against the durable nonsecret policy, and returns a
non-clone `FileCredentialPermit` branded to that exact live `FileOversight` owner.
The operation is read-only with respect to the journal and control authority.

Reopening the durable authority reconstructs the nonsecret policy but creates a
new process-local issuer. A pre-recovery permit is therefore unusable even if a
caller retained its Rust value. Fresh work requires a freshly supplied inventory
and matching broker/provider pair. No secret byte, secret equality result or
credential permit is replayed from disk.

## Execution versus safe nonexecution

A valid first execution under the durable credential policy can occur only through
`publish_checked_with_credential`. The original full-input revalidation, human
co-sign, policy snapshot, deadline, endpoint version and charged-effect accounting
remain mandatory; the credential permit adds no action authority.

The older `publish_checked` operation remains useful for restrictive outcomes. If
fresh evidence is missing or invalid it may seal the original endpoint request
without a credential, allowing the original reconciler to establish nonexecution
and refund the charge. If the evidence would permit execution, the same legacy
operation refuses before endpoint delivery. Historical terminal endpoint receipts
and deadline-based nonexecution likewise do not require re-presenting a secret.
This keeps credential loss from turning outstanding effect accounting into an
unrecoverable dead end.

## Recovery and trust boundary

`FileOversight::open` continues to fence old permissions, cancel only undispatched
work and clear sendable envelopes. Credential mediation does not make a recovered
unknown effect resendable. The persisted policy survives; live secret capabilities
do not.

This remains the operator-controlled synchronous Unix reference profile. The
perimeter inventory and both credential capabilities are trusted host inputs.
Secrets are ordinary process memory, equality is not cryptographic authentication,
and journal storage has no independent authenticity or anti-rollback mechanism.
This is not an encrypted credential vault or a remote provider protocol.

Regression source covers valid credentialed publication, raw/legacy execution
bypass refusal, broker/provider mismatch, credentialless restrictive sealing and
refund, reopen with old-permit rejection plus fresh rebind, ambiguous v1 credential
refusal, and late guard installation refusal. One compile-fail example prevents
credential-permit cloning.

The required RCH runner is unavailable in this environment. Rust compilation,
formatting, Clippy, tests and doctests remain unexecuted; no Beads or production
gate status is changed.
