//! Disclosed numerical probes constrain, but never substitute for, a native
//! helper's judgment over the entire ORIGINAL input. No hidden residual escape.
pub mod peer;

use super::{DisclosedProbe, ReceivedSidecar, SidecarReceiveWork, SidecarReceiver};
use crate::action::consequence::activation::monitor::learned::MAX_LEARNED_MONITOR_COORDINATES;
use crate::action::consequence::activation::probe::{LinearProbe, ProbeOutcome,
    learned::{KvRow, LearnedProbeWork, MAX_CHECKED_KV_PRODUCTS}};
use crate::action::consequence::oversight::{helper_client::native::{
    NativeEvaluationError, NativeEvaluationProgress, NativeEvaluationStatus, NativeEvaluator,
}, helper_workers::wire::WorkerInput};
use crate::round::Verdict;
use crate::Error;
use std::collections::BTreeSet;
use std::fmt;

pub const MAX_SIDECAR_HELPER_PROBES: usize = 256;

/// Independently registered probes, frozen before this worker receives input.
/// This selection is not a claim of all-layer coverage or detector calibration.
#[derive(Clone, Debug)]
pub struct SidecarProbeQuery { pub row: KvRow, pub probe: LinearProbe }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidecarEvaluationError { Contract(Error), Native(NativeEvaluationError) }
impl From<Error> for SidecarEvaluationError {
    fn from(error: Error) -> Self { Self::Contract(error) }
}
impl From<NativeEvaluationError> for SidecarEvaluationError {
    fn from(error: NativeEvaluationError) -> Self { Self::Native(error) }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidecarEvaluationStatus {
    AwaitingInput, Probing, NativeRunning,
    /// A caught unwind cannot restore a prior runnable state.
    Busy,
    Judged(Verdict), Failed(SidecarEvaluationError), Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidecarDecisionBasis { NumericalAlarm, NumericalUncertainty, NativeModel }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SidecarEvaluationProgress {
    pub status: SidecarEvaluationStatus,
    pub revision: u64,
    pub declared_probes: usize,
    pub completed_probes: usize,
    pub receive: Option<SidecarReceiveWork>,
    pub reserved_probes: LearnedProbeWork,
    pub completed_probe_work: LearnedProbeWork,
    pub basis: Option<SidecarDecisionBasis>,
    pub native_started: bool,
    pub native: NativeEvaluationProgress,
}

/// One nonextractable native owner and one independently provisioned disclosure.
/// Every probe is evaluated before any native inference. Alarm yields Hold;
/// unresolved/equal-threshold evidence yields Abstain. Only complete quiet permits
/// the ORIGINAL native evaluator to decide; its failures never become votes.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::sidecar::receiver::native::SidecarNativeEvaluator;
/// fn fallback(worker: &mut SidecarNativeEvaluator) { worker.native_mut(); }
/// ```
pub struct SidecarNativeEvaluator {
    receiver: Option<SidecarReceiver>,
    received: Option<ReceivedSidecar>,
    native: NativeEvaluator,
    queries: Vec<SidecarProbeQuery>,
    budget: LearnedProbeWork,
    reserved: LearnedProbeWork,
    work: LearnedProbeWork,
    receive_work: Option<SidecarReceiveWork>,
    observations: Vec<DisclosedProbe>,
    status: SidecarEvaluationStatus,
    basis: Option<SidecarDecisionBasis>,
    revision: u64,
    native_started: bool,
    cancelled: bool,
}
impl fmt::Debug for SidecarNativeEvaluator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SidecarNativeEvaluator").field("progress", &self.progress()).finish_non_exhaustive()
    }
}
impl SidecarNativeEvaluator {
    pub fn new(native: NativeEvaluator, receiver: SidecarReceiver,
        queries: Vec<SidecarProbeQuery>, budget: LearnedProbeWork) -> Result<Self, Error>
    {
        if native.status() != NativeEvaluationStatus::AwaitingInput || native.position() != 0
            || native.sampled_draws() != 0 { return Err(Error::WrongState); }
        if &native.policy().input_profile != receiver.input_profile() { return Err(Error::Binding); }
        if queries.is_empty() { return Err(Error::InvalidInput); }
        if queries.len() > MAX_SIDECAR_HELPER_PROBES || budget.coordinates > MAX_LEARNED_MONITOR_COORDINATES
            || budget.reconstruction_products > MAX_CHECKED_KV_PRODUCTS { return Err(Error::Limit); }
        let mut unique = BTreeSet::new();
        let mut coordinates = 0_usize;
        for query in &queries {
            let id = query.probe.identity();
            if !unique.insert((query.row, id.id, id.generation)) { return Err(Error::Duplicate); }
            coordinates = coordinates.checked_add(id.dimensions).ok_or(Error::Overflow)?;
        }
        if coordinates > budget.coordinates { return Err(Error::Limit); }
        let mut observations = Vec::new();
        observations.try_reserve_exact(queries.len()).map_err(|_| Error::Limit)?;
        Ok(Self { receiver: Some(receiver), received: None, native, queries, budget,
            reserved: LearnedProbeWork::default(), work: LearnedProbeWork::default(), receive_work: None,
            observations, status: SidecarEvaluationStatus::AwaitingInput, basis: None, revision: 0,
            native_started: false, cancelled: false })
    }
    pub fn progress(&self) -> SidecarEvaluationProgress {
        SidecarEvaluationProgress { status: self.status, revision: self.revision,
            declared_probes: self.queries.len(), completed_probes: self.observations.len(),
            receive: self.receive_work, reserved_probes: self.reserved, completed_probe_work: self.work,
            basis: self.basis, native_started: self.native_started, native: self.native.progress() }
    }
    pub fn observations(&self) -> &[DisclosedProbe] { &self.observations }
    pub fn begin(&mut self, input: &WorkerInput) -> Result<SidecarEvaluationProgress, SidecarEvaluationError> {
        if self.status != SidecarEvaluationStatus::AwaitingInput { return Err(Error::WrongState.into()); }
        self.status = SidecarEvaluationStatus::Busy;
        let result = self.begin_once(input);
        if let Err(error) = result { self.status = SidecarEvaluationStatus::Failed(error); }
        result.map(|()| self.progress())
    }
    fn begin_once(&mut self, input: &WorkerInput) -> Result<(), SidecarEvaluationError> {
        let receiver = self.receiver.take().ok_or(Error::WrongState)?;
        let received = receiver.receive(input)?;
        self.receive_work = Some(received.work());
        self.received = Some(received);
        let received = self.received.as_ref().expect("received disclosure");
        let mut reserved = LearnedProbeWork::default();
        for query in &self.queries { reserved = add(reserved, received.probe_work(&query.probe, query.row)?)?; }
        if reserved.coordinates > self.budget.coordinates
            || reserved.reconstruction_products > self.budget.reconstruction_products { return Err(Error::Limit.into()); }
        self.reserved = reserved;
        self.revision = 1;
        self.status = SidecarEvaluationStatus::Probing;
        Ok(())
    }

    /// One exact probe OR one original native token. The final quiet probe also
    /// admits the complete original text request, without computing a token.
    /// Invalid revisions are free. A failure/unwind cannot repeat this operation.
    pub fn advance(&mut self, expected_revision: u64) -> Result<SidecarEvaluationProgress, SidecarEvaluationError> {
        if expected_revision != self.revision { return Err(Error::Stale.into()); }
        match self.status {
            SidecarEvaluationStatus::Judged(_) => return Ok(self.progress()),
            SidecarEvaluationStatus::Failed(error) => return Err(error),
            SidecarEvaluationStatus::Probing | SidecarEvaluationStatus::NativeRunning => {}
            _ => return Err(Error::WrongState.into()),
        }
        let phase = self.status;
        let revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.status = SidecarEvaluationStatus::Busy;
        self.revision = revision;
        let result = match phase {
            SidecarEvaluationStatus::Probing => self.probe_once(),
            SidecarEvaluationStatus::NativeRunning => self.native_once(),
            _ => unreachable!("admitted phase"),
        };
        if let Err(error) = result { self.status = SidecarEvaluationStatus::Failed(error); }
        result.map(|()| self.progress())
    }
    fn probe_once(&mut self) -> Result<(), SidecarEvaluationError> {
        let query = self.queries.get(self.observations.len()).ok_or(Error::WrongState)?;
        let remaining = LearnedProbeWork {
            coordinates: self.reserved.coordinates.checked_sub(self.work.coordinates).ok_or(Error::Binding)?,
            reconstruction_products: self.reserved.reconstruction_products
                .checked_sub(self.work.reconstruction_products).ok_or(Error::Binding)?,
        };
        let observation = self.received.as_ref().ok_or(Error::Incomplete)?
            .evaluate_probe(&query.probe, query.row, remaining)?;
        self.work = add(self.work, observation.work())?;
        self.observations.push(observation); // capacity reserved at construction
        if self.observations.len() != self.queries.len() {
            self.status = SidecarEvaluationStatus::Probing;
            return Ok(());
        }
        if self.work != self.reserved { return Err(Error::Binding.into()); }
        let alarm = self.observations.iter().any(|p| p.outcome() == ProbeOutcome::CertifiedAlarm);
        let quiet = self.observations.iter().all(|p| p.outcome() == ProbeOutcome::CertifiedQuiet);
        if alarm || !quiet {
            self.native.cancel(); // no action-model computation or default Allow
            self.basis = Some(if alarm { SidecarDecisionBasis::NumericalAlarm } else { SidecarDecisionBasis::NumericalUncertainty });
            self.status = SidecarEvaluationStatus::Judged(if alarm { Verdict::Hold } else { Verdict::Abstain });
        } else {
            self.native_started = true;
            self.native.begin(self.received.as_ref().ok_or(Error::Incomplete)?.input())?;
            self.status = SidecarEvaluationStatus::NativeRunning;
        }
        Ok(())
    }
    fn native_once(&mut self) -> Result<(), SidecarEvaluationError> {
        let progress = self.native.advance(self.native.position())?;
        self.status = match progress.status {
            NativeEvaluationStatus::Judged(verdict) => {
                self.basis = Some(SidecarDecisionBasis::NativeModel);
                SidecarEvaluationStatus::Judged(verdict)
            }
            NativeEvaluationStatus::Running => SidecarEvaluationStatus::NativeRunning,
            _ => return Err(Error::Binding.into()),
        };
        Ok(())
    }
    pub fn evaluate(&mut self, input: &WorkerInput) -> Result<Verdict, SidecarEvaluationError> {
        self.begin(input)?;
        loop {
            let progress = self.advance(self.revision)?;
            if let SidecarEvaluationStatus::Judged(verdict) = progress.status { return Ok(verdict); }
        }
    }
    /// Destroy unfinished numerical ownership and hidden source retention. Keep
    /// completed scores, spent work, genuine failures and final decisions visible.
    pub fn cancel(&mut self) -> bool {
        if self.cancelled { return false; }
        self.cancelled = true;
        self.receiver = None;
        self.received = None;
        self.native.cancel();
        if matches!(self.status, SidecarEvaluationStatus::AwaitingInput | SidecarEvaluationStatus::Probing
            | SidecarEvaluationStatus::NativeRunning) { self.status = SidecarEvaluationStatus::Cancelled; }
        true
    }
}
fn add(a: LearnedProbeWork, b: LearnedProbeWork) -> Result<LearnedProbeWork, Error> {
    Ok(LearnedProbeWork { coordinates: a.coordinates.checked_add(b.coordinates).ok_or(Error::Overflow)?,
        reconstruction_products: a.reconstruction_products.checked_add(b.reconstruction_products).ok_or(Error::Overflow)? })
}
