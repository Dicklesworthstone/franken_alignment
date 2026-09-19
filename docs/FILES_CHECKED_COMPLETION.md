# Concrete-source atomic completion

`FileOversight::complete_publication_from_files` closes the native policy-source
lease gap in atomic completion. It takes the original automatic/human keys,
optional credential, sealed `EvidenceFile` reader, concrete witness reader, optional
concrete feed reader, and trusted clock. No callback supplies policy or committee
evidence on this path. The native file-source profile must already be configured;
interrupted owners and missing profiles refuse before I/O.

The existing completion machine is shared by source-only callbacks, feed callbacks,
and this concrete path. It still performs independent acquisitions before dispatch
and first publication, using only original journal events and the original endpoint.
A feed, when supplied, catches up before the witness revision is selected. Each
policy file is reopened through the existing sealed reader and staged as the native
SourceEvent::Observe. Helper packets are built from that actual observation with
the original inputs_for implementation, not from caller-supplied packet bytes.

## Lease and authority semantics

The policy observation time is sampled BEFORE reading, exactly as in the original
native source adapter. Dispatch and publication check it AFTER capture. Identical
source bytes can renew the native lease without invalidating the prior exact
review. New producer generations, changed policy/contexts, incomplete snapshots
and reader failures retain native invalidation behavior. They cannot install a new
committee packet under the original human/automatic approvals. The first dispatch
references current inputs from the candidate broker, not the older live image.

Before external policy/clock code, the live source-interruption latch and storage
fault latch close admission. The private candidate uses the SAME admission and
acknowledgment functions at its own staged prefix. A staged source observation can
clear only the candidate interruption; the live owner remains closed until final
canonical replacement. This distinction is necessary for valid unchanged-source
completion without exposing a permitting source update before the effect cut.
There is no new authority or alternate source-admission whitelist.

The original durable witness/feed withdrawals precede acquisition. The native
source is NOT unconditionally withdrawn in the journal before an unchanged read:
that would revoke the original human approval before it could be used. Instead
interruption protects the read, and the existing native source observer decides
whether the new observation preserves or invalidates the original basis.

## Failure, visibility and cost

A first policy read/refusal after acquisition begins leaves the owner unavailable;
no source-update acknowledgment or receipt is fabricated. Recovery uses the original
fenced path. After staged dispatch, an ordinary failed policy read stages native
source withdrawal, and the original publisher seals and settles nonexecution.
A successful but changed second source still passes through original publication
revalidation; it cannot rebase a review. Storage, allocation, clock or candidate
errors retain quarantine. A failed final write exposes the earlier withdrawn image
or the whole effect/accounting cut, not independently published policy updates.

FilesCompletionReport separates actual observation identities/read failures from
committed_source_updates. The latter and nested committed feed reports are EMPTY
on completion failure, including when reads succeeded. On success they accompany
the original checked publication and its accepted receipt. The actual reader's
read count and generation floor are not rolled back by transaction failure.

Two policy acquisitions add two native events to existing fixed admission: ten
additional events without a feed, fourteen with a feed, plus each unseen notice.
Every original encoding prefix still checks byte/event and reserved-recovery limits.
The sealed source reader, witness reader and feed keep their existing bounds.
Replay, allocation, encoding and file reads are additional bounded work, not a
claimed wall-clock execution bound or cross-producer atomic snapshot.

`tests/files_checked_completion.rs` contains ten regression tests with a native
leased source, real files, real committee reduction and original two-key accounting.
They pair lease-refresh success with callback/expiry failures, and cover changed
sources, read loss, catch-up, failed replacement, clock unwind, foreign keys and
capacity. These tests and compilation/formatting/Clippy remain UNEXECUTED here:
RCH, cargo and rustc are unavailable. No production qualification or FA-061/062
closure is claimed. Producer authenticity, source completeness and filesystem
control remain host assumptions; this is atomicity for the original canonical
journal-as-publication sink, not arbitrary remote effects.
