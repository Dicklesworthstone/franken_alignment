//! Coupled producer projections in the original private effect transaction.
use super::{Completion, CompletionEvidence, SourceCut, FileOversight, JournalError,
    FileCaptureError, ElapsedTick, failure};
use super::super::FilePublicationCapture;

impl<F, P> Completion<'_, F, P>
where F: FnMut() -> ElapsedTick, P: CompletionEvidence {
    pub(super) fn read_for_cut(&mut self, host: &mut FileOversight, cut: &mut SourceCut)
        -> Result<Result<FilePublicationCapture, FileCaptureError>, JournalError>
    {
        let Some(feed) = self.feed.filter(|feed| self.source.shares_producer(feed)) else {
            return Ok(self.read());
        };
        let (capture, batch) = match self.source.read_coupled(feed) {
            Ok(pair) => pair,
            Err(error) => {
                self.feed_reads.push(Err(error));
                self.reads.push(Err(error));
                return Ok(Err(error));
            }
        };
        self.feed_reads.push(Ok(batch.heartbeat()));
        self.reads.push(Ok(capture.identity()));
        // This observation is NEVER cached across dispatch or publication. It
        // follows committee/policy capture, then applies its own notifications
        // before the caller obtains the resulting witness revision.
        host.fault = Some(failure(false));
        let now = (self.clock)();
        let report = cut.stage_feed(host, &batch, now)?;
        self.feeds.push(report);
        Ok(Ok(capture))
    }
}
