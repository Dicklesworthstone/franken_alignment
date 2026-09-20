# Supervised completion with native file sources

`FileSupervisedDriver::complete_from_files_with_publication` finishes a reviewed
actor job using the sealed native policy/committee file reader, concrete witness
file, optional concrete change-feed file, trusted clock, original independent
human approval, and optional current credential. No committee/policy callback is
accepted. Configure the native file-source and publication profiles before proposals
and bind the original witness recipe before completion.

The driver shares one job/permit routine with the existing source and feed callback
APIs. A private acquisition strategy is the only difference. Authorization delegates
to the original FileProvider, optionally inside FeedProvider and PublicationProvider,
so every source observation follows existing read-start timing, source-generation
floors and durable refusal semantics. A retained automatic permit skips this phase;
it is not reserved or issued a second time.

The driver retires its send phase before calling the host's new concrete atomic
completion. The host then performs two real native policy reads and two witness
reads, and two feed reads when configured through this API. Source observations,
final effect and original receipt settlement become visible in the same final
canonical cut. Normal completion therefore acquires each configured source three
times in total: authorization, dispatch, first publication. Success closes the
settled job; no later publish or reconciliation scheduler turn is needed.

FileFilesCompletionReport keeps separately acknowledged authorization observations,
native source updates, feed reports and witness reads apart from the host completion.
Host observations are actual reads, not installation evidence. Its committed native
source updates and feed reports are empty whenever final completion fails, including
when authorization succeeded and both final file reads completed.

A first completion-feed read failure before policy acquisition can leave a healthy
Authorized owner and the SAME retained permit for real retry. Native policy capture
failures use original withdrawal/quarantine semantics; they do not preserve approval
under changed source data. A failed final write or caught completion clock unwind
leaves the send phase retired. The next completion cannot silently retry or export
an old source as current; use the original recovery/cancellation machinery. Late
source loss can seal and settle native nonexecution without inventing an outcome
for an ambiguous external effect.

This closes the prior integration gap where atomic completion could enforce but
not refresh the native policy-source lease. The callback entry points retain their
explicit lack of native renewal. The concrete API accepts None for the feed to serve
native source-only profiles; independently configured change/freshness gates still
apply and cannot be disabled by that argument. Every actual effect check uses the
original current clock after capture; source start times and producer heartbeat
times are never extended merely to accommodate slow reads.

`tests/file_driver_files_completion.rs` adds eight helper-socket integration tests
covering three-acquisition success, source generation changes and loss, retained
permit retry, live feed/witness catch-up, lease boundary neighbors, ambiguous final
writes, caught clock unwind and foreign human keys. Existing callback completion
and file-step tests remain unchanged. Compilation, formatting, Clippy and all new
Rust tests remain UNEXECUTED pending the required exact-revision RCH gate. No
production qualification or FA-061/062 closure is claimed. Real-world completeness,
producer authenticity, filesystem isolation and the shared elapsed clock remain
host assumptions; sequential file acquisitions are not a distributed transaction.
