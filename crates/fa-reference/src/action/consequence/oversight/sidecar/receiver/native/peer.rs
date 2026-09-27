//! The original helper client sends exactly one numerically constrained result.
//! Generic streams need a bounded/nonblocking host contract, just as HelperClient.
use super::{SidecarEvaluationError, SidecarEvaluationProgress, SidecarEvaluationStatus, SidecarNativeEvaluator};
use crate::action::consequence::oversight::{helper_client::{HelperClient, ClientPhase, ClientProgress},
    helper_workers::{MAX_WORKER_SALT_BYTES, io::WorkerIoError}};
use crate::round::Verdict;
use crate::Error;
use std::io::{Read, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidecarPeerError { Contract(Error), Evaluation(SidecarEvaluationError), Wire(WorkerIoError) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SidecarPeerProgress {
    Transport(ClientProgress),
    Evaluation(Box<SidecarEvaluationProgress>),
    /// Prepared once, not proof that the coordinator accepted a reveal.
    ReplyPrepared(Verdict),
}

/// A worker cannot supply an alternate vote, retune probes or extract its native
/// evaluator. Transport failure drops the connection and preserves spent work.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::sidecar::receiver::native::peer::SidecarNativePeer;
/// use fa_reference::round::Verdict;
/// fn override_vote(peer: &mut SidecarNativePeer<std::io::Cursor<Vec<u8>>>) { peer.respond(Verdict::Allow); }
/// ```
pub struct SidecarNativePeer<S> {
    client: Option<HelperClient<S>>,
    evaluator: SidecarNativeEvaluator,
    salt: Vec<u8>,
    failure: Option<SidecarPeerError>,
}
impl<S: Read + Write> SidecarNativePeer<S> {
    pub fn new(client: HelperClient<S>, evaluator: SidecarNativeEvaluator, salt: Vec<u8>) -> Result<Self, Error> {
        if client.phase() != ClientPhase::ReadingRequest
            || evaluator.progress().status != SidecarEvaluationStatus::AwaitingInput { return Err(Error::WrongState); }
        if salt.len() > MAX_WORKER_SALT_BYTES { return Err(Error::Limit); }
        Ok(Self { client: Some(client), evaluator, salt, failure: None })
    }
    pub fn evaluation(&self) -> SidecarEvaluationProgress { self.evaluator.progress() }
    pub fn failure(&self) -> Option<SidecarPeerError> { self.failure }
    pub fn step(&mut self) -> Result<SidecarPeerProgress, SidecarPeerError> {
        if let Some(error) = self.failure { return Err(error); }
        let mut client = self.client.take().ok_or(SidecarPeerError::Contract(Error::WrongState))?;
        // A caught I/O or inference unwind drops the locally held connection and
        // keeps this failure latched; there can be no frame replay or reroll.
        self.failure = Some(SidecarPeerError::Contract(Error::Incomplete));
        let result = self.step_once(&mut client);
        match &result {
            Ok(_) => { self.client = Some(client); self.failure = None; }
            Err(error) => { self.failure = Some(*error); self.evaluator.cancel(); }
        }
        result
    }
    fn step_once(&mut self, client: &mut HelperClient<S>) -> Result<SidecarPeerProgress, SidecarPeerError> {
        if client.phase() != ClientPhase::NeedsInference {
            return client.step().map(SidecarPeerProgress::Transport).map_err(SidecarPeerError::Wire);
        }
        let progress = self.evaluator.progress();
        let next = if progress.status == SidecarEvaluationStatus::AwaitingInput {
            let input = client.input().ok_or(SidecarPeerError::Contract(Error::Incomplete))?;
            if self.salt.len() > input.salt_limit() { return Err(SidecarPeerError::Contract(Error::Limit)); }
            self.evaluator.begin(input)
        } else { self.evaluator.advance(progress.revision) }.map_err(SidecarPeerError::Evaluation)?;
        if let SidecarEvaluationStatus::Judged(verdict) = next.status {
            client.respond(verdict, &self.salt).map_err(SidecarPeerError::Contract)?;
            return Ok(SidecarPeerProgress::ReplyPrepared(verdict));
        }
        Ok(SidecarPeerProgress::Evaluation(Box::new(next)))
    }
    pub fn cancel(&mut self) -> bool {
        let existed = self.client.take().is_some();
        self.evaluator.cancel();
        existed
    }
}
