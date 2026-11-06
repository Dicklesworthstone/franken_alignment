//! Original registered-source acquisition behind complete Tower admission.
//! The supervisor owns source/time and receives private intake diagnostics; the
//! actor receives only the original publication response and keeps its tickets.

use super::{
    ActorError, ElapsedTick, Error, EvidenceFile, EvidenceIdentity, FileActorSupervisor,
    FileEvidenceReport, FileOversight, FileSupervisedDriver, JournalError,
};
use crate::tower::publication::{PublicationResponse, PublicationTower};
use crate::tower::{TowerAccess, TowerRequest};

/// Host-only preparation diagnostics beside the actor-visible response.
/// `intake: None` means no source preparation was attempted; it is not evidence
/// of admission or source freshness. Source and request remain distinct durable
/// transactions, and a failed reply cannot refund either one.
#[derive(Debug)]
pub struct FileTowerExchange {
    pub response: PublicationResponse,
    pub intake: Option<FileEvidenceReport<EvidenceIdentity>>,
}

impl FileActorSupervisor<FileOversight> {
    /// Decode and authorize a complete Tower document before preparing evidence
    /// for a valid NEW native submission. Polls, disclosures, malformed input,
    /// full ticket sessions and exact/conflicting retries read no source or time.
    /// The mounted port must belong to this exact original owner allocation.
    pub fn exchange_tower_from_file<S, F>(
        &mut self,
        tower: &mut PublicationTower,
        access: &TowerAccess,
        document: &[u8],
        source: &mut S,
        clock: F,
    ) -> Result<FileTowerExchange, JournalError>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        self.exchange_tower_selection_from_file(tower, access, None, document, source, clock)
    }

    /// The typed entry point uses the same native proposal validation before
    /// acquiring any observation. Typed construction cannot bypass wire limits.
    pub fn dispatch_tower_from_file<S, F>(
        &mut self,
        tower: &mut PublicationTower,
        access: &TowerAccess,
        request: TowerRequest,
        source: &mut S,
        mut clock: F,
    ) -> Result<FileTowerExchange, JournalError>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        self.check_source_wire(tower.request_port())?;
        let mut intake = None;
        let response = tower.dispatch_with_admission(access, request, |_, key, _| {
            self.prepare_wire_submission(key, source, &mut clock, &mut intake)
        });
        Ok(FileTowerExchange { response, intake })
    }

    /// Restrict new submissions to the operator-selected native request, just
    /// like the original selected-request peer ingress. A foreign Submit key is
    /// withheld before source/time acquisition or any preinstalled snapshot use.
    /// Reads retain the Tower's existing scope, ticket and disclosure rules.
    pub fn exchange_tower_request_from_file<S, F>(
        &mut self,
        tower: &mut PublicationTower,
        access: &TowerAccess,
        selected: u64,
        document: &[u8],
        source: &mut S,
        clock: F,
    ) -> Result<FileTowerExchange, JournalError>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        self.exchange_tower_selection_from_file(
            tower,
            access,
            Some(selected),
            document,
            source,
            clock,
        )
    }

    /// Observe and retry the SAME recorded request during review or recovery.
    /// This interface accepts no source or clock and cannot admit new work,
    /// including when an unused admission snapshot was installed beforehand.
    /// Retained retries still compare the entire original proposal in the owner.
    pub fn exchange_tower_request_observe(
        &mut self,
        tower: &mut PublicationTower,
        access: &TowerAccess,
        selected: u64,
        document: &[u8],
    ) -> Result<PublicationResponse, JournalError> {
        self.check_tower_selection(tower, Some(selected))?;
        Ok(
            tower.exchange_with_admission(access, document, |_, key, _| {
                if key != selected {
                    return Err(ActorError::Withheld);
                }
                match self.host().and_then(|host| host.request_status(key)) {
                    Ok(_) => Ok(()),
                    Err(_) => Err(ActorError::Unavailable),
                }
            }),
        )
    }

    fn exchange_tower_selection_from_file<S, F>(
        &mut self,
        tower: &mut PublicationTower,
        access: &TowerAccess,
        selected: Option<u64>,
        document: &[u8],
        source: &mut S,
        mut clock: F,
    ) -> Result<FileTowerExchange, JournalError>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        self.check_tower_selection(tower, selected)?;
        let mut intake = None;
        let response = tower.exchange_with_admission(access, document, |_, key, _| {
            if selected.is_some_and(|selected| key != selected) {
                return Err(ActorError::Withheld);
            }
            self.prepare_wire_submission(key, source, &mut clock, &mut intake)
        });
        Ok(FileTowerExchange { response, intake })
    }

    fn check_tower_selection(
        &self,
        tower: &PublicationTower,
        selected: Option<u64>,
    ) -> Result<(), JournalError> {
        if selected == Some(0) {
            return Err(Error::InvalidInput.into());
        }
        self.check_source_wire(tower.request_port())
    }
}

impl FileSupervisedDriver {
    /// Original host intake with the driver's existing helper maintenance on
    /// every outcome. Neither actor requests nor the Tower acquire the driver.
    pub fn exchange_tower_from_file<S, F>(
        &mut self,
        tower: &mut PublicationTower,
        access: &TowerAccess,
        document: &[u8],
        source: &mut S,
        clock: F,
    ) -> Result<FileTowerExchange, JournalError>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        let result = self
            .supervisor_mut()
            .exchange_tower_from_file(tower, access, document, source, clock);
        self.reap_helpers();
        result
    }

    pub fn dispatch_tower_from_file<S, F>(
        &mut self,
        tower: &mut PublicationTower,
        access: &TowerAccess,
        request: TowerRequest,
        source: &mut S,
        clock: F,
    ) -> Result<FileTowerExchange, JournalError>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        let result = self
            .supervisor_mut()
            .dispatch_tower_from_file(tower, access, request, source, clock);
        self.reap_helpers();
        result
    }

    pub fn exchange_tower_request_from_file<S, F>(
        &mut self,
        tower: &mut PublicationTower,
        access: &TowerAccess,
        selected: u64,
        document: &[u8],
        source: &mut S,
        clock: F,
    ) -> Result<FileTowerExchange, JournalError>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        let result = self
            .supervisor_mut()
            .exchange_tower_request_from_file(tower, access, selected, document, source, clock);
        self.reap_helpers();
        result
    }

    pub fn exchange_tower_request_observe(
        &mut self,
        tower: &mut PublicationTower,
        access: &TowerAccess,
        selected: u64,
        document: &[u8],
    ) -> Result<PublicationResponse, JournalError> {
        let result = self
            .supervisor_mut()
            .exchange_tower_request_observe(tower, access, selected, document);
        self.reap_helpers();
        result
    }
}
