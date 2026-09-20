//! Private strategies; job/key transitions remain in one original driver routine.
use super::{Callback, CapturedCompletionKeys, CommitteeContract, DriverEvidence,
    ElapsedTick, Error, EvidenceProvider, FeedCompletionReport, FrozenAction,
    JournalError, PublicationFeedFile, PublicationInputFile};
use crate::action::consequence::delivery::persistent::observed::FileOversight;

pub(super) trait CompletionDriverEvidence: EvidenceProvider {
    fn preflight(&self, _host: &FileOversight) -> Result<(), Error> { Ok(()) }
    fn complete<F: FnMut() -> ElapsedTick>(&mut self, host: &mut FileOversight,
        revision: u64, keys: CapturedCompletionKeys<'_>, source: &PublicationInputFile,
        feed: Option<&PublicationFeedFile>, clock: &mut F) -> FeedCompletionReport;
}
impl<P> CompletionDriverEvidence for Callback<P>
where P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error> {
    fn complete<F: FnMut() -> ElapsedTick>(&mut self, host: &mut FileOversight,
        revision: u64, keys: CapturedCompletionKeys<'_>, source: &PublicationInputFile,
        feed: Option<&PublicationFeedFile>, clock: &mut F) -> FeedCompletionReport
    {
        match feed {
            Some(feed) => host.complete_publication_from_feed(revision, keys, source, feed, clock, &mut self.0),
            None => FeedCompletionReport { reads: Vec::new(), committed: Vec::new(),
                completion: host.complete_publication_from_source(revision, keys, source, clock, &mut self.0) },
        }
    }
}

pub(super) struct BorrowedProvider<'a, P>(pub(super) &'a mut P);
impl<P: EvidenceProvider> EvidenceProvider for BorrowedProvider<'_, P> {
    fn capture<F: FnMut() -> ElapsedTick>(&mut self, host: &mut FileOversight,
        action: &FrozenAction, clock: &mut F) -> Result<Result<DriverEvidence, Error>, JournalError>
    {
        self.0.capture(host, action, clock)
    }
}
