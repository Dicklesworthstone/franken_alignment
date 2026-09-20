//! Constant-span ingestion of a complete contiguous observation interval.
//! This is not a closing marker or a way to skip an unobserved prefix.
use super::{FrontierStage, ProductFrontiers, ProjectionFrontiers, ProjectionKey, StageFrontier};
use crate::Error;

impl ProductFrontiers {
    /// Record EVERY position in the inclusive interval [first, last] at one
    /// stage. The caller asserts that the whole interval was observed under that
    /// stage's existing trust contract. A range endpoint alone is not that claim.
    ///
    /// The interval must touch the existing contiguous prefix (or be wholly
    /// within it). Missing predecessors refuse without mutation; no hole is
    /// inferred away. Equivalent to ascending individual accepts on admitted
    /// intervals, including draining already pending successors. Work depends on
    /// bounded pending entries, not the numerical length of the interval.
    ///
    /// Closing observations, stage/projection identities and terminal ceilings
    /// remain independent. Even an authenticated batch never creates closure.
    pub fn accept_contiguous(&mut self, key: ProjectionKey, stage: FrontierStage,
        first: u64, last: u64) -> Result<(), Error>
    {
        if first == 0 || first > last { return Err(Error::InvalidInput); }
        if let Some(projection) = self.projections.get_mut(&key) {
            if projection.close.is_some_and(|marker| last > marker.final_sequence) {
                return Err(Error::WrongState);
            }
            return projection.stages.get_mut(&stage).ok_or(Error::Missing)?
                .accept_contiguous(first, last);
        }
        if first != 1 { return Err(Error::Incomplete); }
        if self.projections.len() >= self.max_streams { return Err(Error::Limit); }
        let mut projection = ProjectionFrontiers::new(self.max_gap)?;
        projection.stages.get_mut(&stage).ok_or(Error::Missing)?.accept_contiguous(first, last)?;
        self.projections.insert(key, projection);
        Ok(())
    }
}

impl StageFrontier {
    fn accept_contiguous(&mut self, first: u64, last: u64) -> Result<(), Error> {
        let prefix = self.frontier.contiguous();
        if last <= prefix { return Ok(()); }
        // last > prefix proves prefix < u64::MAX, so the checked increment
        // cannot fail on a legitimate extension (including a terminal MAX).
        if first > prefix.checked_add(1).ok_or(Error::Overflow)? {
            return Err(Error::Incomplete);
        }
        // Remove only positions explicitly included in the observed interval.
        // Each remaining successor still advances solely on its own observation.
        self.frontier.pending.retain(|position| *position > last);
        self.frontier.contiguous = last;
        while let Some(next) = self.frontier.contiguous.checked_add(1) {
            if !self.frontier.pending.remove(&next) { break; }
            self.frontier.contiguous = next;
        }
        self.greatest_accepted = self.greatest_accepted.max(last);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
