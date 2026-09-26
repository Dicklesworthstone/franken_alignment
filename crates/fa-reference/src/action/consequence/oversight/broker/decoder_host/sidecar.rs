//! Actual hosted KV evidence into the existing whole-input congress, not votes.
//! The source is selected by the owner, never supplied as an external cache.
mod refinement;

use super::OversightBroker;
use crate::action::consequence::activation::monitor::learned::model::LearnedAuditPreparationBudget;
use crate::action::consequence::activation::probe::learned::{CheckedLearnedKv, KvGroup, ResidualRetention};
use crate::action::consequence::activation::tensor::kv::model::learned::{CompressionReport, LearnedKvCodec};
use crate::action::consequence::oversight::{CommitteeInput, sidecar::{SidecarCommitteeRound,
    SidecarCongressBudget, SidecarCongressPlan, SidecarIdentity}};
use crate::Error;
use std::fmt;
use std::rc::Rc;

/// Supervisor-selected codec and bounded disclosure policy. Only preparation
/// inputs are caller-supplied; the complete original KV prefix comes from the
/// broker's actual numerical owner. Origin/codec training labels remain trusted.
#[derive(Clone, Debug)]
pub struct HostedSidecarRequest {
    pub codec: LearnedKvCodec,
    pub evaluation_origin: u64,
    pub retention: ResidualRetention,
    pub preparation: LearnedAuditPreparationBudget,
    pub identity: SidecarIdentity,
    pub priority: Vec<KvGroup>,
    pub budget: SidecarCongressBudget,
}

/// One source-bound congress input sequence. Historical packet access is not a
/// claim of current eligibility: current_input checks the original owner/gate.
/// The planner cannot be extracted, cloned or retuned to reset its budget.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::decoder_host::sidecar::HostedSidecar;
/// fn bypass(sidecar: HostedSidecar) { sidecar.into_plan(); }
/// ```
/// ```compile_fail,E0308
/// use fa_reference::action::{Permit, consequence::oversight::decoder_host::sidecar::HostedSidecar};
/// fn authorize(sidecar: HostedSidecar) -> Permit { sidecar }
/// ```
pub struct HostedSidecar {
    issuer: Rc<()>,
    attempt: u64,
    actor_revision: u64,
    input_revision: u64,
    plan: SidecarCongressPlan,
    current: SidecarCommitteeRound,
    compression: CompressionReport,
}
impl fmt::Debug for HostedSidecar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostedSidecar").field("attempt", &self.attempt)
            .field("actor_revision", &self.actor_revision)
            .field("input_revision", &self.input_revision).finish_non_exhaustive()
    }
}
impl HostedSidecar {
    pub fn attempt(&self) -> u64 { self.attempt }
    pub fn actor_revision(&self) -> u64 { self.actor_revision }
    pub fn input_revision(&self) -> u64 { self.input_revision }
    pub fn source(&self) -> &CheckedLearnedKv { self.plan.source() }
    pub fn compression(&self) -> &CompressionReport { &self.compression }
    /// Immutable historical evidence only; use the broker's checked accessor
    /// before treating this packet as the current proposal's helper input.
    pub fn round(&self) -> &SidecarCommitteeRound { &self.current }
}
impl OversightBroker {
    /// Snapshot and source-check the ACTUAL hosted decoder's full accepted KV,
    /// produce the original coarse sidecar packet, and record it through the
    /// existing input-revision transaction. No helper runs or verdict is invented.
    /// This optional supervisor observation does not replace compulsory residual
    /// monitoring, the current-source gate, human keys or publication checks.
    pub fn begin_hosted_sidecar(&mut self, attempt: u64, actor_revision: u64,
        input_revision: u64, request: HostedSidecarRequest) -> Result<HostedSidecar, Error>
    {
        self.check_hosted_sidecar_source(attempt, actor_revision)?;
        if self.input_revision(attempt)? != input_revision { return Err(Error::Stale); }
        let action = self.inputs.get(&attempt).ok_or(Error::Missing)?.action.clone();
        let host = self.decoder_host.as_ref().ok_or(Error::Incomplete)?;
        let (source, compression) = host.run.capture_sidecar(&request.codec, request.evaluation_origin,
            request.retention, request.preparation)?;
        let mut plan = SidecarCongressPlan::new(source, request.identity, request.priority, request.budget)?;
        let current = plan.initial(&action, &self.contracts)?;
        // All fallible numerical and packet work precedes the original input
        // commit. A failed observation does not delete the previous input or
        // manufacture a monitor verdict. No callback can advance the source.
        let revision = self.record_inputs(attempt, input_revision, current.input().clone())?;
        Ok(HostedSidecar { issuer: Rc::clone(&self.issuer), attempt, actor_revision,
            input_revision: revision, plan, current, compression })
    }

    /// Current source and exact whole-input version, not merely matching IDs or
    /// stored packet bytes. Original begin/apply/authorize/dispatch also check the
    /// same decoder proposal basis; callers cannot bypass it with old evidence.
    pub fn current_hosted_sidecar(&self, sidecar: &HostedSidecar) -> Result<&CommitteeInput, Error> {
        self.check_hosted_sidecar(sidecar)?;
        self.current_inputs(sidecar.attempt)?.ok_or(Error::Incomplete)
    }

    fn check_hosted_sidecar_source(&self, attempt: u64, actor_revision: u64) -> Result<(), Error> {
        if self.inspect().suspended || self.stop_receipt().is_some() { return Err(Error::WrongState); }
        if self.actor_revision() != actor_revision { return Err(Error::Stale); }
        if self.decoder_host.is_none() { return Err(Error::Incomplete); }
        self.check_decoder(attempt)
    }
    fn check_hosted_sidecar(&self, sidecar: &HostedSidecar) -> Result<(), Error> {
        if !Rc::ptr_eq(&sidecar.issuer, &self.issuer) { return Err(Error::Binding); }
        self.check_hosted_sidecar_source(sidecar.attempt, sidecar.actor_revision)?;
        if self.input_revision(sidecar.attempt)? != sidecar.input_revision
            || self.current_inputs(sidecar.attempt)? != Some(sidecar.current.input()) {
            return Err(Error::Stale);
        }
        Ok(())
    }
}
