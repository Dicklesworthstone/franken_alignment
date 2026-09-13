# Durable external requests through full-input, two-key oversight

Consumer: an actor-facing adapter for FileOversight. The persistent owner already
replays original whole-input reviews, human approvals and endpoint outcomes; this
increment makes the external request identity part of that same durable history.
It implements the integration of the request boundary (FA-107; plan sections 8,
16, 17) with the existing independently co-signed publication profile.

## Original request book, original oversight authority

FileDelivery and FileOversight share the same bounded RequestBook. It validates
exact external keys and original action structure, allocates IDs independently of
actor keys, and records the RESULT of the original host's proposal method. The
simpler host still calls DeliveryBroker; the full-input host calls OversightBroker.
No request record can supply a verdict, bypass a review, or create either key.

The full-input journal explicitly admits the existing nested SubmitRequest tag.
Its original bootstrap, helper contracts, mandatory human policy, and old event
bytes are unchanged. The nested single-key authorize/dispatch and plain ballot
operations remain forbidden. Older readers reject the newly admitted request tag
rather than treating it as an original proposal or silently upgrading a profile.

Exact retries compare every action field and return the latest durable request
projection without another event or admission, even without a fresh snapshot or
clock. Changed fields conflict. Structurally valid admission refusals remain
recorded and their internal allocations are not recycled. The 128-record/2-MiB
request retention allowance includes refusals and terminal requests; journal and
original broker caps independently apply. There is no eviction on cancellation.

Request projections refresh from the original broker after every successful
journal transition, including an outer-success/inner-refusal review completion.
All private review and reservation stages remain one visible generation class.
No separate rights or terminal-outcome reducer is introduced.

## Recovery cannot import either approval

Reopening still withdraws old human keys, advances the original authority epoch,
cancels only undispatched reservations, and fences the endpoint before returning
a new owner. The request map replays with it. Retrying an old request can recover
its observation, not its review session, reservation, human approval or envelope.
Fresh work needs a new external key and the current original control path.

Dispatched uncertainty remains charged. Cancellation or exact retry does not
refund it. Reconciliation and stop draining remain available without helper or
reviewer availability and use the original endpoint's receipts. A changed whole
helper input or expired human permit remains a dispatch refusal after exact retry.

## Verification boundary

Six integration-test functions cover full-input two-key publication and lost-ack
recovery, reservation/key nonresurrection with fresh-epoch positive publication,
input drift and human expiry, retained admission refusals, mixed stop obligations,
and shared request limits plus complete field binding. Existing FileDelivery
request tests and original FileOversight tests are unchanged. Rust compilation,
formatting and execution have not run; the required RCH runner is unavailable.
This is source integration, not a production or revision-bound qualification.

Storage/clock/peer provenance and human authentication remain operator assumptions.
The full hosted decoder and its recovery state are not imported into this journal.
No dependency, executor, original reducer, global status, Beads state or historical
execution evidence changes as part of this increment.
