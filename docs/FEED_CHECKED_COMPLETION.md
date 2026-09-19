# Feed-aware atomic publication completion

`FileOversight::complete_publication_from_feed` connects actual change-window
catch-up to the original source-completion path. It addresses the composition
failure where the ordinary driver can acquire the feed, but a composite effect
must independently refresh it before dispatch and before first publication.
The API lives in `observed::publication::capture::completion::feed` and borrows
`CapturedCompletionKeys`, a concrete witness reader and a concrete feed reader.
No new event type, permission, endpoint, dependency or rights ledger is introduced.

## Execution and visibility

The original action must already be reviewed, automatically authorized and
independently human-approved. Required credentials must be from the same live
owner and generation. Wrong keys, source bindings, profile absence and insufficient
fixed event capacity refuse before reads. The host first commits feed withdrawal,
then witness withdrawal, before external I/O, callbacks or clock code.

On a private replay of the SAME broker and endpoint, completion reads the feed,
validates every overlapping record against retained history and applies only its
unseen contiguous suffix and original heartbeat. It obtains the witness revision
AFTER those notifications may have withdrawn the active attempt. It then captures
committee/policy evidence, reads the concrete witness file, samples time and stages
original two-key dispatch. The second boundary withdraws staged feed eligibility,
rereads the actual feed, then starts a new witness acquisition at the resulting
revision. Original checked publication and receipt reconciliation complete the cut.

The overlap checker is shared with ordinary live feed ingestion. For the second
read it includes the FIRST staged feed history, not only the old live journal.
A producer cannot rewrite a newly observed sequence between these two reads.
Covered-record presence after the independent bootstrap remains checked; a later
window cannot silently fill an omitted prefix. Missing-window and repair rules
remain those of the original change/heartbeat gate.

Only one final canonical replacement exposes the staged changes, heartbeat
observations, effect outcome and settled accounting. During callbacks a concurrent
canonical reader sees the earlier withdrawals and undispatched reservation, not
a speculative charge or publication. Two successful acquisitions with no new
notices add 12 events: two preliminary withdrawals and ten final-cut events.
Every new notice adds one event; a failed second feed read omits its observation.
The earlier callback-only source-completion API retains its existing behavior.

## Failure and diagnostics

`FeedCompletionReport.reads` retains at most two actual heartbeat identities or
file errors. They are not installation receipts. `committed` is EMPTY whenever
completion fails; successful entries are exposed only after the entire final cut
is acknowledged. `completion` retains the original witness-read diagnostics,
committee failure and checked publication/settlement result. Historical read-time
eligibility is not a permit: the original final gate checks time after capture.

Missing first feed input leaves both durable withdrawals in place and the original
keys/reservation unspent. A healthy owner can retry with the SAME keys and genuinely
new reads. Once a concrete feed has been read successfully, any later admission,
clock, overlap, allocation, replay or storage failure quarantines the live owner.
This includes a first observed expired or incomplete feed that prevents dispatch;
its unacknowledged candidate cannot be forgotten by returning to older coverage.
Use original fenced recovery, not an automatic redispatch or a replacement key.

After staged dispatch, ordinary second-feed read loss leaves feed eligibility
withdrawn while actual committee/witness capture remains separate. The original
publisher seals and accepts its nonexecution receipt in the same final cut.
A detected history conflict remains an installation failure, not a fabricated
successful seal. Original execution deadlines, credential checks and settlement
rules remain independent. A failed final write exposes either the old withdrawn
cut or the entire completed cut, never a reported partially installed suffix.

## Scope and verification

Both feed reads retain the original 256-record/32-KiB bounds and both witness
reads retain their original limits. Fixed event capacity is checked before I/O;
actual suffix capacity and every original encoding/recovery-reserve prefix are
checked before the corresponding candidate transition. There is no unbounded
retry or fallback to partial overlap results. Replay, allocation, capture and
history comparison remain additional bounded work, not claimed CPU-time bounds.

Feed, witness and policy reads are sequential, not an atomic cross-producer
snapshot. Producer completeness, authenticity, filesystem control and the shared
elapsed clock remain host assumptions. The committee callback does NOT renew a
separately configured native policy-source lease; that stronger gate still refuses
expired data. This API is for the original journal-as-publication sink, not a
transaction across arbitrary remote effects.

`tests/feed_checked_completion.rs` adds 11 real-file/committee/two-key tests for
successful catch-up, post-dispatch dependency changes with unrelated controls,
conflicting staged overlap, lost/late windows, expiry, caught clock unwind, final
write failure, both capacity boundaries and foreign inputs. Existing source and
ordinary feed tests remain independent regression coverage. Rust compilation,
formatting, Clippy and tests remain UNEXECUTED here: RCH/cargo/rustc are unavailable.
The required exact-revision RCH xtask gate remains outstanding; FA-061/062 stay open.
