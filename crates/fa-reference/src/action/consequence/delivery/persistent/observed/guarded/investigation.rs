//! Historical admission/review diagnosis through the ORIGINAL durable machine.
//! No source refresh, helper inference, writer lock, recovery fence or effect.
use super::{FileRecoveryRequirements, FileOversight, FileOversightProfile, JournalError,
    Machine, Event, BaseEvent, journal, storage};
use super::super::replay::FileReviewReplay;
use super::super::FileDeliverySnapshot;
use crate::action::Purpose;
use crate::action::consequence::experiment::{Intervention, InterventionScope, PolicyExperiment};
use crate::action::consequence::experiment::proposal::{ProposalExperiment, ProposalCounterfactualReport};
use crate::action::consequence::experiment::proposal::search::{ProposalRepairCursor, ProposalRepairReport,
    ProposalSearchBudget, ProposalSearchStatus, ProposalSearchWork};
use crate::action::consequence::gate::containment::session::policy::controller::Proposal;
use crate::action::consequence::oversight::replay::ObservedReviewAnchor;
use crate::Error;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Data at TWO cuts. The initial Reviewing/Denied admission must not be confused
/// with a later cancellation, authorization, dispatch or terminal outcome.
/// The proposal includes the original policy and its actual compiled witnesses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileProposalOrigin {
    directory: PathBuf,
    external_request: Option<u64>,
    proposal: Proposal,
    scope: InterventionScope,
    admitted: FileDeliverySnapshot,
    journal: FileDeliverySnapshot,
}
impl FileProposalOrigin {
    pub fn directory(&self) -> &Path { &self.directory }
    pub fn external_request(&self) -> Option<u64> { self.external_request }
    pub fn proposal(&self) -> &Proposal { &self.proposal }
    pub fn scope(&self) -> &InterventionScope { &self.scope }
    pub fn admission_snapshot(&self) -> &FileDeliverySnapshot { &self.admitted }
    pub fn journal_snapshot(&self) -> &FileDeliverySnapshot { &self.journal }
}

/// A policy repair has no effect key, new source observation or authority to
/// reopen the original denied attempt. Its origin stays historical after use.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::investigation::FileProposalDiagnosis;
/// fn approve(report: FileProposalDiagnosis) -> FilePermit { report }
/// ```
#[derive(Clone, Debug)]
pub struct FileProposalDiagnosis {
    origin: Rc<FileProposalOrigin>,
    result: ProposalCounterfactualReport,
}
impl FileProposalDiagnosis {
    pub fn purpose(&self) -> Purpose { Purpose::Experiment }
    pub fn origin(&self) -> &FileProposalOrigin { &self.origin }
    pub fn result(&self) -> &ProposalCounterfactualReport { &self.result }
}
#[derive(Clone, Debug)]
pub struct FileProposalRepairReport {
    origin: Rc<FileProposalOrigin>,
    search: ProposalRepairReport,
}
impl FileProposalRepairReport {
    pub fn purpose(&self) -> Purpose { Purpose::Experiment }
    pub fn origin(&self) -> &FileProposalOrigin { &self.origin }
    pub fn search(&self) -> &ProposalRepairReport { &self.search }
}
#[derive(Clone, Debug)]
pub struct FileProposalInvestigation {
    origin: Rc<FileProposalOrigin>,
    experiment: ProposalExperiment,
}
impl FileProposalInvestigation {
    pub fn purpose(&self) -> Purpose { Purpose::Experiment }
    pub fn origin(&self) -> &FileProposalOrigin { &self.origin }
    pub fn run(&self, edits: &[Intervention]) -> Result<FileProposalDiagnosis, Error> {
        Ok(FileProposalDiagnosis { origin: Rc::clone(&self.origin), result: self.experiment.run(edits)? })
    }
    pub fn begin_repair_search(&self, candidates: &[Intervention], budget: ProposalSearchBudget)
        -> Result<FileProposalRepairSearch, Error>
    {
        Ok(FileProposalRepairSearch { origin: Rc::clone(&self.origin),
            cursor: self.experiment.begin_repair_search(candidates, budget)? })
    }
}

/// No host, endpoint, role, saved random state or active review is retained.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::investigation::FileProposalRepairSearch;
/// fn duplicate(search: FileProposalRepairSearch) { let _ = search.clone(); }
/// ```
#[derive(Debug)]
pub struct FileProposalRepairSearch {
    origin: Rc<FileProposalOrigin>,
    cursor: ProposalRepairCursor,
}
impl FileProposalRepairSearch {
    pub fn status(&self) -> ProposalSearchStatus { self.cursor.status() }
    pub fn work(&self) -> ProposalSearchWork { self.cursor.work() }
    pub fn advance(&mut self) -> Result<ProposalSearchStatus, Error> { self.cursor.advance() }
    pub fn cancel(&mut self) -> Result<(), Error> { self.cursor.cancel() }
    pub fn report(&self) -> FileProposalRepairReport {
        FileProposalRepairReport { origin: Rc::clone(&self.origin), search: self.cursor.report() }
    }
    pub fn finish(&self) -> Result<FileProposalRepairReport, Error> {
        Ok(FileProposalRepairReport { origin: Rc::clone(&self.origin), search: self.cursor.finish()? })
    }
}

impl FileOversight {
    /// Supervisor investigation of an ACKNOWLEDGED admission, including hard
    /// denials that never reached a congress. It can outlive a stopped owner.
    /// Refused requests with no frozen action do not acquire a fictional proposal.
    pub fn investigate_proposal(&self, attempt: u64, scope: InterventionScope)
        -> Result<FileProposalInvestigation, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        extract(self.store.identity(), &self.profile, &self.events, attempt, scope, None)
    }

    /// Read one whole canonical image without touching its lock or staging data.
    /// The SAME guarded-recovery contract pins numeric inputs before replay,
    /// then all gates/current policy/history floors before returning any report.
    /// This performs no recovery. Historical reconstruction is extra work, not
    /// part of the subsequent bounded subset-search budget.
    pub fn read_proposal_investigation(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileRecoveryRequirements, attempt: u64, scope: InterventionScope)
        -> Result<FileProposalInvestigation, JournalError>
    {
        expected.guards.validate()?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let events = journal::decode(profile, &identity, &bytes)?;
        extract(&identity, profile, &events, attempt, scope, Some(expected))
    }
}

impl FileReviewReplay {
    /// Historical full-input congress diagnosis requires the independently
    /// retained pre-vote anchor, not only the inner policy transcript. A replayed
    /// Continue may still have a committed application refusal; this does not
    /// change application(), revive a key, or predict votes on edited inputs.
    pub fn policy_experiment(&self, expected: &ObservedReviewAnchor, scope: InterventionScope)
        -> Result<PolicyExperiment, Error>
    {
        if expected != self.anchor() { return Err(Error::Binding); }
        self.archive().verify(expected)?;
        if let Ok(receipt) = self.application() { self.archive().verify_receipt(expected, receipt)?; }
        PolicyExperiment::from_archive(&self.archive().policy, &expected.policy, scope)
    }
}

fn extract(directory: &Path, profile: &FileOversightProfile, events: &[Event], attempt: u64,
    scope: InterventionScope, expected: Option<&FileRecoveryRequirements>)
    -> Result<FileProposalInvestigation, JournalError>
{
    if attempt == 0 { return Err(Error::InvalidInput.into()); }
    if let Some(expected) = expected { expected.guards.check_decoder_config(events)?; }
    let mut machine = Machine::new(profile)?;
    let mut selected = None;
    for (index, event) in events.iter().enumerate() {
        // A request key need not equal its allocated attempt. Observe the FIRST
        // creation in the original action map, not an assumed ID mapping or a
        // later retry. All refusal/recovery/source/numerical transitions still run.
        let source = if selected.is_none() && !machine.actions.contains_key(&attempt) {
            match event {
                Event::Core(BaseEvent::Propose(_, _, snapshot)) => Some((None, snapshot)),
                Event::Core(BaseEvent::SubmitRequest(request, _, snapshot)) => Some((Some(*request), snapshot)),
                _ => None,
            }
        } else { None };
        machine.apply(event)?;
        if let Some((request, snapshot)) = source {
            if let Some(action) = machine.actions.get(&attempt) {
                let admitted = machine.snapshot(index + 1);
                let policy = Rc::new(machine.policy_updates.current(&profile.delivery.policy).clone());
                // The native machine has already accepted THIS frozen action and
                // disposition. Re-evaluate its original input snapshot to retain
                // the actual trace, checking the compiled witnesses in new().
                let proposal = Proposal { attempt, action: action.clone(),
                    evaluation: policy.evaluate(action, snapshot)?, policy,
                    snapshot_semantic_epoch: snapshot.semantic_epoch,
                    state: *admitted.control.ledger.stages.get(&attempt).ok_or(Error::Missing)? };
                let experiment = ProposalExperiment::new(proposal, snapshot, scope.clone())?;
                selected = Some((request, admitted, experiment));
            }
        }
    }
    // Never return early at a valid prefix: malformed/contradictory suffixes and
    // independently pinned ending policy or guards must still refuse the image.
    if let Some(expected) = expected { expected.check(profile, &machine, events)?; }
    let (external_request, admitted, experiment) = selected.ok_or(Error::Missing)?;
    let origin = Rc::new(FileProposalOrigin { directory: directory.to_owned(), external_request,
        proposal: experiment.original().clone(), scope, admitted, journal: machine.snapshot(events.len()) });
    Ok(FileProposalInvestigation { origin, experiment })
}
