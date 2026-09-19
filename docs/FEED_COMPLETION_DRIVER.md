# Supervised feed-aware completion

`FileSupervisedDriver::complete_with_publication_feed` completes the existing
reviewed actor job with real feed catch-up at authorization, dispatch and first
publication. It borrows the original independent human permit, optional current
credential, concrete witness/feed readers, trusted clock and committee callback.
The original review and source binding must already exist. No actor command,
new event format, effect owner, executor or permission type is introduced.

The driver now shares one private completion routine between this entry point
and `complete_with_publication_source`. The older API retains its callback-only
acquisition behavior and result type; it does not silently acquire a feed.
Before any new acquisition the routine checks original ownership, Ready state,
human/source/credential bindings, and the configured feed/freshness profile.

When the original automatic key is absent, authorization uses the SAME
FeedProvider -> PublicationProvider -> committee callback path as ordinary feed
steps. Catch-up therefore precedes selection of the current witness revision.
The existing sample/authorize operation creates one reservation and one permit.
With a retained permit, this phase is skipped rather than charging/reserving again.

The driver then retires its send phase and invokes the original host's
`complete_publication_from_feed`. That performs two new feed/witness acquisitions
and installs its original dispatch, checked publication and receipt settlement in
one final canonical replacement. On success the driver closes the settled job;
there is no additional publish or reconciliation turn and subsequent steps are Idle.
The preliminary withdrawals and automatic authorization are separate acknowledged
operations, not part of an asserted single transaction over the whole method.

`FileFeedCompletionReport`, in `driver::evidence::publication::completion`, keeps
independently acknowledged `authorization_feeds` and `authorization_reads` apart
from the host `completion` report. A normal successful call has one authorization
feed/witness read and two completion feed/witness reads. Host `committed` reports
are empty on failure even when actual feed reads succeeded. Only the final
`completion.completion.result` acknowledges a completed and settled effect.

An ordinary first completion-feed read failure can leave the healthy owner
Authorized. The driver then retains the SAME key/reservation and restores Ready;
a retry performs real new reads but does not authorize a second time. A detected
history conflict, failed installation, ambiguous final write or caught callback
unwind leaves the send path retired. Recovery uses the original fenced owner;
there is no automatic reissue, receipt invention or refund of an unknown effect.
Late feed loss can seal through the original endpoint and settle its real
nonexecution receipt. Gap repair and producer expiry remain independent of the
unchanged committee packet; a feed outage is not mislabeled as committee drift.

This is still a callback for committee/policy capture. It does not renew a
separately configured native policy-source lease, authenticate file producers,
or create an atomic snapshot across files. All stronger native checks remain
mandatory. The existing three-reader `step_from_files_with_publication_feed`
remains the path for durable native policy-source refreshes; it retains its
separate-transition semantics. The new completion path is not a remote transaction.

`tests/file_driver_feed_completion.rs` adds eight real helper-socket integration
tests: complete three-acquisition publication, post-dispatch changes with unrelated
controls, same-permit retry after feed loss, late loss/settlement, gap repair,
conflicting staged history, final expiry/policy drift, and caught provider unwind.
Existing callback-only completion and ordinary feed tests are unchanged. All new
Rust tests, compilation, formatting and Clippy remain UNEXECUTED pending the
repository's RCH verifier on the exact revision. Source/whitespace checks are not
runtime evidence; FA-061 and FA-062 remain open.
