//! Replay measurements through the ORIGINAL identity observer and authority.
//! No decoded event supplies a match verdict, balance or containment receipt.
use super::{Machine, Transition};
use super::super::identity::{FileIdentityObservation, IdentityEvent};
use crate::action::consequence::activation::identity::ModelPassport;
use crate::action::consequence::oversight::identity::{IdentityChallenge, IdentityInstallation, IdentityObserver, IdentityOutcome, IdentityPolicy, IdentityReport};
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

pub(super) struct IdentityState {
    passport: Rc<ModelPassport>,
    policy: IdentityPolicy,
    observer: IdentityObserver,
    challenges: BTreeMap<u64, IdentityChallenge>,
}

impl Machine {
    pub(in super::super) fn identity_contract(&self) -> Option<(&ModelPassport, IdentityPolicy)> {
        self.identity.as_ref().map(|state| (state.passport.as_ref(), state.policy))
    }

    pub(super) fn apply_identity(&mut self, event: &IdentityEvent) -> Result<Transition, Error> {
        if !matches!(event, IdentityEvent::Enable(..) | IdentityEvent::Unavailable(_)) && !self.clock_ready {
            return Err(Error::Incomplete);
        }
        match event {
            IdentityEvent::Enable(passport, policy) => {
                if self.identity.is_some() { return Err(Error::Duplicate); }
                if !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
                    || self.broker.inspect().sequence != 0 || self.broker.stop_receipt().is_some()
                { return Err(Error::WrongState); }
                let observer = self.broker.enable_identity_checks(passport.as_ref().clone(), *policy)?;
                if !self.publication_guard { self.enable_publication_guard()?; }
                self.identity = Some(IdentityState { passport: Rc::clone(passport), policy: *policy,
                    observer, challenges: BTreeMap::new() });
                Ok(Transition::Unit)
            }
            IdentityEvent::Begin(id, sequence, actor_revision) => {
                if self.identity.is_none() { return Err(Error::Incomplete); }
                let result = self.broker.begin_identity_check(*id, *sequence, *actor_revision);
                if let Ok(challenge) = &result {
                    self.identity.as_mut().expect("configured identity").challenges.insert(*id, challenge.clone());
                }
                // The native capacity refusal can withdraw a previous live basis.
                // Preserve it as an acknowledged result, not a rolled-back error.
                Ok(Transition::IdentityBegun(result))
            }
            IdentityEvent::Manifest(id, manifest, at) => {
                let challenge = self.identity_challenge(*id)?;
                self.observe(*at)?;
                let result = self.identity.as_ref().expect("retained challenge").observer
                    .observe_manifest(&challenge, manifest.clone(), *at);
                self.identity_observed(&challenge, result)
            }
            IdentityEvent::Anchor(id, anchor, frame, at) => {
                let challenge = self.identity_challenge(*id)?;
                self.observe(*at)?;
                let result = self.identity.as_ref().expect("retained challenge").observer
                    .observe_anchor(&challenge, *anchor, frame, *at);
                self.identity_observed(&challenge, result)
            }
            IdentityEvent::Apply(id, sequence, epoch) => {
                let challenge = self.identity_challenge(*id)?;
                let installed = self.install_identity(&challenge, *sequence, *epoch)?;
                Ok(Transition::IdentityApplied(Box::new(installed)))
            }
            IdentityEvent::Unavailable(basis) => {
                self.broker.identity_unavailable(*basis)?;
                self.withdraw_keys()?;
                Ok(Transition::Unit)
            }
        }
    }

    fn identity_challenge(&self, id: u64) -> Result<IdentityChallenge, Error> {
        self.identity.as_ref().ok_or(Error::Incomplete)?.challenges.get(&id).cloned().ok_or(Error::Missing)
    }

    fn identity_observed(&mut self, challenge: &IdentityChallenge, measurement: Result<IdentityReport, Error>)
        -> Result<Transition, Error>
    {
        let containment = if measurement.as_ref().is_ok_and(|report| matches!(report.outcome, IdentityOutcome::Mismatch(_))) {
            let control = self.broker.inspect();
            // Even a failed suspension must preserve the observed mismatch latch.
            // Original first-publication checks refuse while that latch is set.
            Some(self.install_identity(challenge, control.sequence, control.ledger.epoch))
        } else { None };
        Ok(Transition::IdentityObserved(Box::new(FileIdentityObservation { measurement, containment })))
    }

    fn install_identity(&mut self, challenge: &IdentityChallenge, sequence: u64, epoch: u64)
        -> Result<IdentityInstallation, Error>
    {
        let installed = self.broker.apply_identity_check(challenge, sequence, epoch)?;
        for id in &installed.cancelled { self.automatic.remove(id); }
        if matches!(installed.report.outcome, IdentityOutcome::Mismatch(_)) {
            self.sessions.clear();
            self.withdraw_keys()?;
        }
        // Original envelopes survive solely for guarded publication/sealing and
        // reconciliation. An identity incident is never a nonexecution receipt.
        Ok(installed)
    }

    /// Recovery cannot restore matching eligibility from saved observations.
    /// A never-clocked bootstrap has no live basis to withdraw. Mismatch and
    /// per-anchor sequence floors remain in the original gate after withdrawal.
    pub(super) fn withdraw_identity(&mut self) -> Result<(), Error> {
        if self.identity.is_some() && self.broker.inspect().ledger.elapsed.is_some() {
            self.broker.identity_unavailable(self.broker.identity_basis()?)?;
        }
        Ok(())
    }
}
