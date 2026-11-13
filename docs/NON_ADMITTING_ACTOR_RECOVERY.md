# Recover existing actor tickets without admitting replacement actions

`FileActorPort::recovery_only()` produces `FileActorRecoveryPort`, usable by the
original ActorWire, ActorChannel, UnixActorConnection and Linux PeerSession.
It accepts the original Submit representation only for a request already
retained by the same original FileDelivery or FileOversight owner. The ordinary
gateway compares every original proposal field before returning a ticket.
Missing records are Withheld, not a fictional NotAdmitted or Absent outcome.
No source, clock, snapshot, allocator, new journal encoding, or alternate
outcome reducer is added.

The wrapper first reads the actual sealed host's request book. These owners
never delete or reassign a retained key, and the same non-Send gateway retains
that owner while the original retry runs. There is no external callback or
scheduling point between checks. A new key cannot consume a preinstalled
one-use snapshot. A retained downstream refusal is recoverable even without
a frozen admitted action. Faulted owners refuse rather than return their older
acknowledged state.

## Select one request or a finite set

`for_request` further narrows the capability to one independently selected key
across Submit, Poll and Cancel. It cannot widen or switch to another key.
The existing `selected_request()` observation retains its single-key meaning.

`FileActorRecoveryPort::for_requests(&[u64])` consumes that recovery port and
returns a distinct `FileActorRecoverySetPort`. This supports exact recovery of
several independently selected requests through one original wire session:

```rust
let selected = port.recovery_only().for_requests(&[701, 700])?;
assert_eq!(selected.selected_requests(), &[700, 701]);
let only_700 = selected.clone().for_request(700)?;
```

The set is nonempty and contains at most `MAX_RECOVERY_REQUESTS` (64) unique,
nonzero keys. Empty selections, zero keys and duplicates return
`Error::InvalidInput`; overlong selections return `Error::Limit`. All structural
and membership checks precede allocation or access to the owner. The private
selection is sorted; `selected_requests()` reports these configured identities,
not an enumeration of retained records or a claim that the records exist.

A previously selected single-key port can produce only that same singleton
set. The set's own `for_requests` and `for_request` methods can only narrow its
current selection. Adding or retargeting a key returns `Error::Binding`.
Selection does not reserve request identities. A selected missing key remains
Withheld when submitted; it cannot create a record. Unselected Submit, Poll and
Cancel operations are withheld before owner lookup or request disclosure.
Selected operations delegate to the original non-admitting port, including its
complete proposal comparison and ticket-issuer checks.

Both port types keep weak custody of the same original owner. Clones share
that owner and cannot keep the authority lock alive after its supervisor is
dropped. A ticket from another gateway remains withheld. There is no
admitting-port unwrap. Narrowing one value does not revoke independently
retained broader clones or ordinary ports held by the supervisor.

## Cancellation, transport and the runnable service

Cancel still invokes the original cancellation rule, so these capabilities are
not fully read-only. Only an undispatched reservation can be released; sent and
executed liabilities remain charged, and an unknown endpoint outcome stays
Unknown until original reconciliation. Recovering a ticket does not resend an
effect, refund a charge, recover old approval keys or supply an execution permit.

Connection authentication and exact server/domain provisioning are still
required. Numeric IDs alone cannot recover tickets, and peer credentials do
not attest the server's executable or persistent storage. The wire format is
unchanged: a receiver is independently configured in this restricted mode;
an arbitrary admitting server does not acquire it because a client intends
recovery.

The existing [receipt-only actor service](RECEIPT_ONLY_ACTOR_SERVICE.md)
provides the runnable single-request recovery workflow. It accepts exact
resubmission of an unanswered original request at that explicitly restricted
receiver without creating a new action if no server record exists. This does
not weaken the durable cancellation command's requirement for a recorded
acknowledgment when connecting to an ordinary admitting service.

## Verification boundary

The original six regression functions cover missing keys with a preserved
snapshot and admitting positive control; every changed proposal field,
single-key narrowing and foreign tickets; cancellation before/after dispatch
and execution; retained admission refusals through FileDelivery; real Unix
lost-reply recovery of an executed request; and unavailable or borrowed owners.

Four additional public functions cover finite selection across multiple
original requests, exact retries and foreign tickets, a selected missing key
that preserves the original admission snapshot, invalid and maximum-sized
selections, nonwidening descendants, independent weak clones, and unchanged
session-local ticket recovery through the existing encoded ActorWire. A
two-request case performs actual full-input review, separate human approval,
dispatch, publication and reconciliation for one request, then verifies that
cancelling the other unsent request releases only its reservation while the
executed charge remains. Both original durable host types are exercised.
Each recovery port type has a compile-fail boundary rejecting conversion to
an admitting capability.

The finite-selection increment has independent source review only. The required
Rust toolchain and RCH are unavailable in the editing environment; compilation,
tests, rustfmt and Clippy remain unverified, and no passing runtime result is claimed. These changes
advance the FA-107/FA-014 reference recovery integration without claiming
whole-packet or roadmap qualification.
