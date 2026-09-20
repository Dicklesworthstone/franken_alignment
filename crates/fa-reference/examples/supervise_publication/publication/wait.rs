//! Bounded orchestration only: the native capture and effect gates decide.
use super::{PreparedPublication, FileDriverEvent, FileSupervisedDriver,
    FileEvidenceSource, FileHumanPermit, ElapsedTick, debug};

const MAX_RETRIES: u64 = 64;
#[derive(Clone, Copy, Debug)]
pub(super) struct WaitPolicy { max_retries: u8 }
impl WaitPolicy {
    pub(super) fn new(max_retries: u64) -> Result<Self, String> {
        if max_retries == 0 || max_retries > MAX_RETRIES {
            return Err("producer max_retries must be in 1..=64".into());
        }
        Ok(Self { max_retries: max_retries as u8 })
    }
}

pub(super) struct WaitBudget { remaining: u8 }
impl WaitBudget {
    pub(super) fn new(policy: WaitPolicy) -> Self { Self { remaining: policy.max_retries } }
    fn retry(&mut self) -> Result<(), String> {
        self.remaining = self.remaining.checked_sub(1)
            .ok_or("producer catch-up retry budget exhausted")?;
        Ok(())
    }
}

impl PreparedPublication<'_> {
    /// None means an acknowledged native deferral, never a successful event.
    /// Return to the original loop so it checks its unchanged logical/wall
    /// deadline and services independent stop control before any next read.
    pub fn step_or_wait<F>(&mut self, driver: &mut FileSupervisedDriver,
        evidence: &mut FileEvidenceSource, time: &mut F, human: Option<&FileHumanPermit>)
        -> Result<Option<FileDriverEvent>, String>
    where F: FnMut() -> ElapsedTick {
        if self.wait.is_none() { return self.step(driver, evidence, time, human).map(Some); }
        let report = driver.step_from_files_with_publication_deferral(evidence,
            self.current.reader(), self.feed.map(|feed| &feed.reader), time, human, None);
        if let Some(observed) = report.waiting_for_producer() {
            self.wait.as_mut().ok_or("missing producer wait policy")?.retry()?;
            eprintln!("Publication remains undispatched pending producer catch-up: {:?}", observed.outcome);
            return Ok(None);
        }
        report.step.publication.evidence.result.map(Some).map_err(debug)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_retry_budget_has_no_unbounded_or_saturating_neighbor() {
        for limit in 1..=MAX_RETRIES {
            let mut budget = WaitBudget::new(WaitPolicy::new(limit).unwrap());
            for _ in 0..limit { budget.retry().unwrap(); }
            assert!(budget.retry().is_err()); assert!(budget.retry().is_err());
            assert_eq!(budget.remaining, 0);
        }
        for limit in [0, MAX_RETRIES + 1, u64::MAX] { assert!(WaitPolicy::new(limit).is_err()); }
    }
}
