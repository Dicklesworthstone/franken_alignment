# Persistent terminal stop and original endpoint drain

This extends the coupled Unix `FileDelivery` reference host. The base publication
and recovery contract is in `FILE_DELIVERY_RECOVERY.md`; pending-effect batch
reconciliation is in `FILE_DELIVERY_SWEEP.md`. This is not full OversightBroker
recovery, authenticated transport, or a production foundation admission.

## Permanently close this publication domain

`FileDelivery::request_stop(revision, StopRequest)` calls the ORIGINAL
`DeliveryBroker::request_stop`. The exact operation ID, control sequence and
authority epoch are checked by that original transaction. It cancels and refunds
only undispatched reservations, revokes old authority, marks interrupted dispatches
Unknown, and advances the dispatcher epoch. A stop does not require a current
clock reading, a new policy snapshot, or a helper's permission.

The coupled file host then discards its private permits and unsent envelopes.
These envelopes never escaped to an external transport: the host owns the only
publication sink. Discarding them withdraws their delivery path but is NOT a
nonexecution proof or a refund. Already-published requests and their original
endpoint receipts remain retained. The local stop and its effects on admission
are committed through the existing canonical journal transaction before the
original StopReceipt is returned.

No resume operation exists. Reopening replays the original terminal stop rather
than inferring one from counters. Its normal additional recovery fence preserves
the stop; neither fresh clock observation, generic fencing, a different process,
nor a new facade permit revives intake. Exact stop-request retries return the
original receipt even after the predecessor moves. Reusing the operation ID with
changed fields or submitting a different stop operation follows the original
broker's binding/duplicate refusal. Retries still consume journal slots.

`FileDeliverySnapshot::stop` and `stop_receipt()` expose the original local-stop
receipt as historical data. They do not assert that the endpoint has been fenced
or that an outstanding effect did not execute. Like ordinary inspection, these
reads can lag disk following a failed replacement. `stop_progress()` refuses an
unavailable storage owner rather than presenting its older cut as fresh progress.

## Settle outstanding effects without reopening intake

`FileDelivery::progress_stop(revision, tick)` admits the explicit current logical
tick at both original broker and endpoint, then calls the ORIGINAL
`DeliveryBroker::progress_stop`. It can therefore be used immediately after
recovery without treating a stored historical tick as current. A rollback refuses
before committing. Clock-domain continuity and correctness remain operator trust.

The original stop sweep installs and acknowledges the CURRENT dispatcher fence,
looks up each unresolved recoverable request, and seals missing requests without
waiting for their original execution deadline. A recorded execution wins over a
seal and stays charged. Expired retention remains unresolved and charged; it is
not converted to nonexecution. The returned `FileStopSweep` preserves every
per-attempt result and the original `StopProgress`, including unresolved IDs,
reserved/charged units, fence status and `drained()` semantics. A drained stop can
retain charges for successfully executed effects; drain does not undo them.

The candidate stop sweep runs only in the private memory projection. Its clock,
fence, settlements and original accounting are persisted together before any
result is returned. A storage failure exposes no candidate refund and leaves the
owner unavailable. Reopening uses the canonical cut and retains any already
committed stop. An inert pending file never becomes a stop or refund receipt.

Tags 11 and 12 encode the exact stop request and stop-progress tick in the
existing version-one framing. No balance, endpoint outcome or caller-supplied
stop receipt is serialized as authoritative input. The unchanged event and byte
limits still apply, including to stop and recovery. A full journal may block
further drain or writable recovery; read-only history retains the stop and its
liabilities rather than inventing a new emergency budget.

## Source scenarios and verification status

Five public integration tests add mixed reserved/unsent/published stop recovery,
exact predecessor/idempotency checks without a clock, retention-expired and still
live sibling obligations, real staging-file failure during drain, and terminal
stop at the journal ceiling. They include repeated owner reconstruction, old-key
refusal, current-fence advancement, fresh-time refusal, irreversible intake closure,
and original execution receipts winning over sealing. Existing assertions and
underlying reducers are unchanged; no dependency or unsafe code was added.

The required RCH runner is absent in this editing environment. The RCH invocation
failed with `rch: command not found`; no local compilation fallback was used.
These new Rust tests and the implementation have NOT been compiled or executed.
Formatting, Clippy and the full revision-bound gate remain pending. No Beads task
or production gate is closed on the strength of source-level test construction.

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_delivery_stop
```

All base-profile trust limits remain: plaintext operator-owned storage, cooperative
kernel locking, no hostile-namespace guarantee, no authenticated journal or
independent same-path anti-rollback anchor, and bounded but quadratic history
replay/rewrite. This connects terminal stop and drain to the existing file-backed
reference consumer; it does not claim complete production crash qualification.
