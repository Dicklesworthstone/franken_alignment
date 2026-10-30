# Declared-fleet publication situations

This extends FA-136 / plan 17.8 with a read-only composition of the original
publication situation readers. It is not full-system situation coverage, an
atomic fleet snapshot, a planner, a shutdown coordinator or a live action grant.

## Native reader

`replay::situation::fleet::FileSituationFleetReplay::new` accepts a fixed roster
of nonzero member IDs and optional pristine `FileSituationReplay` cursors.
Select each ordinary, learned-generation or owned-learned-predictor reader using
its original API and independent configuration. A failed input is represented by
`None`, retaining that member as Unknown rather than fabricating an empty journal.
No failed member is retried through an ordinary, unanchored or weaker reader.

Admission rejects an empty roster, duplicate/zero IDs, overlarge rosters, excessive
aggregate event counts, and already advanced or failed cursors before applying
any transition. The absolute limits are 64 members and 1,048,576 admitted events;
callers can select smaller limits, including zero events for empty journals.
These limits do not bound bytes already loaded by a caller. The original member
source and recipe byte limits, plus aggregate intake and elapsed allowances, must
also be enforced by the consumer. Numerical events remain synchronous.

`advance(n)` spends at most n original events across the entire roster, with a
maximum of 64 per call. Sorted member IDs determine round-robin replay order.
A long journal cannot starve a short journal; an invalid transition retires only
that member. The member is marked unavailable before original work starts, so
catching an unwind cannot revive its suspect prefix. Progress reports attempted,
checked and admitted events separately. Admitted events include unread suffixes
of subsequently rejected members; Ready does not mean every member was verified.

`finish` refuses while any member remains pending. Otherwise it drops all replay
machines and returns immutable `FileSituationFleet` data. Each known member was
fully checked through its own original reducer and retains its exact independent
read basis. No owner, sampler, observation lease, helper session, role or permit
is exported. Dropping unfinished replay cancels private memory work only.

## Pages and coverage

Each fleet page carries the same declared/known/unknown member counts and one
member's original bounded situation page, or explicit Unknown plus a fixed
failure category. Missing reads sort first, followed by unresolved outcomes,
pending work and other histories, with member IDs breaking ties. This is lifecycle
triage, not learned relevance. Within a known member, original attempt ordering,
whole-cut accounting and pagination are unchanged. Empty valid histories appear.

Use the OUTER fleet page's next token to enumerate the roster. The inner next
token covers that member only. Both are immutable capture-branded data; tokens
from a different capture are refused even when revisions happen to match.
Completing enumeration does not establish complete knowledge. Check
`coverage().all_members_known()` separately. A fully read journal can itself
contain unresolved or irrecoverably unknown effects; those remain unresolved.

There is no global journal revision, clock or sum of possibly unrelated balances.
Members may have different scopes, currencies and acquisition times. The roster
is caller-declared, not an authenticated inventory of every real worker/domain.
An independent anchor checks its exact member prefix, not authenticity of an
unanchored suffix. No report changes after a later live write or reconciliation.

## Verification status

Eight new native regression functions use the existing real canonical-history
fixtures and original two-key construction. They cover round-robin work bounds,
missing and semantically invalid members alongside valid histories, lifecycle
ordering, nested pages, exact independent bases, immutable captures and foreign
tokens, every incomplete replay boundary, admission limits/preconsumed inputs,
empty-vs-unreadable members and anchored rollback without weaker retry.

The targeted RCH test and Cargo formatting attempts could not start in this
editing environment: `rch` and `cargo` are absent (exit 127). Tests are authored,
not executed; source review and Git blob preservation are not compilation or
runtime qualification. No bead or full FA-136 acceptance condition is closed.
