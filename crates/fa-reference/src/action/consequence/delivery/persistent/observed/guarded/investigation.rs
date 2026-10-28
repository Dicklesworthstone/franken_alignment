//! Historical admission/review diagnosis through the ORIGINAL durable machine.
//! No source refresh, helper inference, recovery fence or effect. Base readers
//! avoid the writer lock; learned numerical recovery keeps its exclusive lock.
use super::{FileRecoveryRequirements, FileOversight, FileOversightProfile, JournalError,
    Machine, Event, BaseEvent, journal, storage};
use super::super::replay::FileReviewReplay;
use super::super::FileDeliverySnapshot;
use crate::action::Purpose;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
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
/// Historical policy input and experiments only. A diagnosis cannot supply a
/// live owner, approval or effect permit, including after learned replay.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::investigation::FileProposalInvestigation;
/// fn recover_owner(report: FileProposalInvestigation) -> FileOversight { report }
/// ```
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::investigation::FileProposalInvestigation;
/// fn approve(report: FileProposalInvestigation) -> FilePermit { report }
/// ```
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

// Both eager and cooperative readers select the FIRST creation through the
// same original machine. This state is private until the whole image qualifies.
#[derive(Clone, Copy)]
enum ProposalIdentity { Attempt(u64), Request(u64) }
pub(in crate::action::consequence::delivery::persistent::observed) struct Selection {
    identity: ProposalIdentity,
    scope: InterventionScope,
    selected: Option<(Option<u64>, FileDeliverySnapshot, ProposalExperiment)>,
}
impl Selection {
    fn new(attempt: u64, scope: InterventionScope) -> Result<Self, Error> {
        if attempt == 0 { return Err(Error::InvalidInput); }
        Ok(Self { identity: ProposalIdentity::Attempt(attempt), scope, selected: None })
    }
    pub(in crate::action::consequence::delivery::persistent::observed) fn request(request: u64, scope: InterventionScope) -> Result<Self, Error> {
        if request == 0 { return Err(Error::InvalidInput); }
        Ok(Self { identity: ProposalIdentity::Request(request), scope, selected: None })
    }
    pub(in crate::action::consequence::delivery::persistent::observed) fn apply(&mut self, profile: &FileOversightProfile, machine: &mut Machine,
        index: usize, event: &Event) -> Result<(), JournalError>
    {
        // A durable actor key is NOT its allocated attempt. Resolve only the
        // first submission through the original request book, including a
        // retained NotAdmitted disposition. Neither a direct proposal with the
        // same number nor a later request/retry may substitute another action.
        let source = if self.selected.is_none() {
            match self.identity {
                ProposalIdentity::Attempt(attempt) if !machine.actions.contains_key(&attempt) => match event {
                    Event::Core(BaseEvent::Propose(_, _, snapshot)) => Some((None, snapshot)),
                    Event::Core(BaseEvent::SubmitRequest(request, _, snapshot)) => Some((Some(*request), snapshot)),
                    _ => None,
                },
                ProposalIdentity::Request(request) if matches!(machine.requests.status(request), Err(Error::Missing)) => match event {
                    Event::Core(BaseEvent::SubmitRequest(key, _, snapshot)) if *key == request => Some((Some(request), snapshot)),
                    _ => None,
                },
                _ => None,
            }
        } else { None };
        machine.apply(event)?;
        if let Some((request, snapshot)) = source {
            let attempt = match self.identity {
                ProposalIdentity::Attempt(attempt) => attempt,
                ProposalIdentity::Request(key) => match machine.requests.status(key)?.disposition {
                    FileRequestDisposition::Admitted { attempt, .. } => attempt,
                    FileRequestDisposition::NotAdmitted(_) => return Ok(()),
                },
            };
            if let Some(action) = machine.actions.get(&attempt) {
                let admitted = machine.snapshot(index + 1);
                let policy = Rc::new(machine.policy_updates.current(&profile.delivery.policy).clone());
                let proposal = Proposal { attempt, action: action.clone(),
                    evaluation: policy.evaluate(action, snapshot)?, policy,
                    snapshot_semantic_epoch: snapshot.semantic_epoch,
                    state: *admitted.control.ledger.stages.get(&attempt).ok_or(Error::Missing)? };
                let experiment = ProposalExperiment::new(proposal, snapshot, self.scope.clone())?;
                self.selected = Some((request, admitted, experiment));
            }
        }
        Ok(())
    }
    fn require_selected(&self) -> Result<(), Error> {
        if self.selected.is_some() { Ok(()) } else { Err(Error::Missing) }
    }
    pub(in crate::action::consequence::delivery::persistent::observed) fn finish(self, directory: &Path, machine: &Machine, count: usize)
        -> Result<FileProposalInvestigation, JournalError>
    {
        let (external_request, admitted, experiment) = self.selected.ok_or(Error::Missing)?;
        let origin = Rc::new(FileProposalOrigin { directory: directory.to_owned(), external_request,
            proposal: experiment.original().clone(), scope: self.scope, admitted,
            journal: machine.snapshot(count) });
        Ok(FileProposalInvestigation { origin, experiment })
    }
}

fn extract(directory: &Path, profile: &FileOversightProfile, events: &[Event], attempt: u64,
    scope: InterventionScope, expected: Option<&FileRecoveryRequirements>)
    -> Result<FileProposalInvestigation, JournalError>
{
    let mut selection = Selection::new(attempt, scope)?;
    if let Some(expected) = expected { expected.guards.check_decoder_config(events)?; }
    let mut machine = Machine::new(profile)?;
    for (index, event) in events.iter().enumerate() {
        selection.apply(profile, &mut machine, index, event)?;
    }
    // Never return early at a valid prefix: the complete suffix and independent
    // ending policy, guards and floors must still qualify the captured image.
    if let Some(expected) = expected { expected.check(profile, &machine, events)?; }
    selection.finish(directory, &machine, events.len())
}

mod replay;
pub use replay::{FileProposalRead, FileProposalReadProgress, FileProposalReadStatus};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod request_tests;
