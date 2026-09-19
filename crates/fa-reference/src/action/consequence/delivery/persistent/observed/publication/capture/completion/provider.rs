//! Private acquisition strategies for the original completion transaction.
//! No implementation or speculative owner is exposed to embedding callers.
use super::{DriverEvidence, ElapsedTick, Error, FileOversight, FrozenAction, JournalError, SourceCut};
use crate::action::consequence::oversight::CommitteeContract;

pub(super) trait CompletionEvidence {
    fn preflight(&self, _host: &FileOversight) -> Result<(), Error> { Ok(()) }
    fn fixed_events(&self) -> usize { 0 }
    fn capture<F: FnMut() -> ElapsedTick>(&mut self, host: &mut FileOversight,
        cut: &mut SourceCut, action: &FrozenAction, clock: &mut F)
        -> Result<Result<DriverEvidence, Error>, JournalError>;
}

pub(super) struct CallbackEvidence<P>(pub(super) P);
impl<P> CompletionEvidence for CallbackEvidence<P>
where P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error> {
    fn capture<F: FnMut() -> ElapsedTick>(&mut self, host: &mut FileOversight,
        _cut: &mut SourceCut, action: &FrozenAction, _clock: &mut F)
        -> Result<Result<DriverEvidence, Error>, JournalError>
    {
        Ok((self.0)(action, &host.profile.committee))
    }
}
