# Durable two-key request completion

These APIs serve trusted supervisors owning `FileOversight`; they are not actor
commands, remote-provider transactions, new authority ledgers, or a relaxation of
any configured publication guard.

`complete_credentialed_publication` consumes existing automatic/human approvals
and a live `FileCredentialPermit`. It stages the original Time, Dispatch,
PublishCredentialed and Reconcile events, checks every prefix against the original
journal/recovery bounds, then replaces the canonical publication once. Credential
brands, perimeter policy and current unrevoked generation are checked live; secrets
never become replay data. Storage failure quarantines the owner before it can
reuse an ambiguous dispatch opportunity.

`complete_request_publication` additionally binds a durable external request ID
to those keys and the entire frozen action. A new completion delegates to the
original checked or credentialed path. A same-owner retry of already settled work
returns its accepted historical outcome without another write, credential
exercise or dispatch. Foreign/old-process keys and changed action bytes refuse,
even on the historical branch. Missing mandatory credentials cannot select a
weaker publication route. Source-bound attempts still use the existing
`complete_publication_from_source` for two independent acquisitions.

`request_resolution` reads accepted original endpoint evidence by external request
ID, including after restart and after later credential or policy revocation. It
requires no live effect keys. **None is not proof of nonexecution.** In particular,
the endpoint may already have published while the broker has not accepted a
receipt. Unknown or retention-expired deliveries remain charged. Refused and
pre-dispatch-cancelled requests also have no endpoint receipt; `request_status`
keeps those dispositions distinct. A faulted owner returns Unavailable instead of
presenting its old acknowledged state as current.

After an ambiguous acknowledgment, reopen with the independently bound profile
and a fresh clock observation using `open_reconciled_publication`, inspect every
pending reconciliation result, then query `request_resolution`. Never create a
new request ID or resend an unknown dispatch merely because no receipt is known.

## Verification status

The changes add nine credential-completion tests and eleven request-completion
and receipt-recovery tests. They include permitted executions as well as foreign
keys, altered action bytes, revoked credentials, stale observations, uncertain
outcomes, exact terminal retries, restart, capacity and all five deterministic
storage barriers. They use real canonical file replacement and original reducers;
the injected barriers do not simulate hardware power loss.

The required RCH invocation could not start in this editing environment (`rch:
command not found`, exit 127). Rust token/delimiter checks passed, but compilation,
rustfmt, Clippy, the tests and the full xtask gate remain **UNEXECUTED**. No
production qualification or bead closure follows from these source changes.
