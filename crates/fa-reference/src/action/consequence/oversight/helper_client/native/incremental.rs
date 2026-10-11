//! Cooperatively schedule the ORIGINAL tokenizer and monitored helper. Complete
//! prompt admission precedes numerical work; a partial answer is no vote.

use super::{NativeEvaluationError, NativeEvaluationStatus, NativeEvaluator,
    TextDecoder, TextGenerationRequest, WorkerInput, GenerationFinish};
use crate::action::consequence::activation::monitor::decoder::MonitoringWork;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::text::TextGenerationAdmission;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::text::incremental::TextGenerationSession;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{TokenizationProgress, TokenizationWork};
use crate::action::consequence::activation::tensor::kv::decoder::DecoderWork;
use crate::Error;

/// Original numerical-owner counters, never an independent budget or refund
/// ledger. After an unwind the observed position/coverage can lag actual work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeEvaluationWork {
    pub position: u64,
    pub sampled_draws: u64,
    pub decoder: DecoderWork,
    pub monitoring: MonitoringWork,
}

/// Scheduling diagnostics, with no answer bytes, token IDs, logits or effect key.
/// Only Judged contains a verdict, after the ORIGINAL complete-output reducer.
/// A finish on Failed/Cancelled is historical progress, never a permitting result.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::oversight::helper_client::native::NativeEvaluationProgress;
/// use fa_reference::round::Verdict;
/// fn early_vote(progress: NativeEvaluationProgress) -> Verdict { progress }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeEvaluationProgress {
    pub status: NativeEvaluationStatus,
    pub work: NativeEvaluationWork,
    /// Original tokenizer counters, retained on admission failure/cancellation.
    /// These do not count allocations or the separately bounded initial scans.
    pub tokenization: TokenizationWork,
    /// Cooperative tokenizer advance attempts, including a failed/interrupted
    /// final attempt. Zero for the existing synchronous-admission entry point.
    pub admission_steps: usize,
    /// Zero while Admitting: no partial prompt token count is presented as final.
    pub requested_prompt_tokens: usize,
    pub reviewed_prompt_tokens: usize,
    pub released_answer_tokens: usize,
    pub finish: Option<GenerationFinish>,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Snapshot {
    work: NativeEvaluationWork,
    tokenization: TokenizationWork,
    requested: usize,
    reviewed: usize,
    released: usize,
    finish: Option<GenerationFinish>,
}

pub(super) enum Execution {
    Decoder(Box<TextDecoder>),
    Admitting(Box<TextGenerationAdmission>),
    Running(Box<TextGenerationSession>),
    /// A diagnostic copy after destruction; cannot recreate numerical ownership.
    Closed(Snapshot),
}

impl NativeEvaluator {
    fn snapshot(&self) -> Snapshot {
        match &self.execution {
            Execution::Admitting(admission) => Snapshot {
                work: NativeEvaluationWork { position: admission.position(), sampled_draws: admission.sampled_draws(),
                    decoder: admission.decoder_work(), monitoring: admission.monitoring_work() },
                tokenization: admission.tokenization_work(),
                ..Snapshot::default()
            },
            Execution::Running(run) => {
                let p = run.progress();
                Snapshot {
                    work: NativeEvaluationWork { position: run.position(), sampled_draws: run.sampled_draws(),
                        decoder: run.decoder_work(), monitoring: run.monitoring_work() },
                    tokenization: run.prompt().work(),
                    requested: p.requested_prompt_tokens(), reviewed: p.reviewed_prompt_tokens(),
                    released: p.tokens().len(), finish: p.finish(),
                }
            }
            Execution::Decoder(decoder) => {
                let mut snapshot = Snapshot { work: NativeEvaluationWork {
                    position: decoder.position(), sampled_draws: decoder.sampled_draws(),
                    decoder: decoder.decoder_work(), monitoring: decoder.monitoring_work(),
                }, ..Snapshot::default() };
                if let Some(report) = &self.report {
                    let p = report.generation();
                    snapshot.tokenization = report.prompt().work();
                    snapshot.requested = p.requested_prompt_tokens();
                    snapshot.reviewed = p.reviewed_prompt_tokens();
                    snapshot.released = p.tokens().len();
                    snapshot.finish = Some(p.finish());
                }
                snapshot
            }
            Execution::Closed(snapshot) => *snapshot,
        }
    }

    pub fn work(&self) -> NativeEvaluationWork { self.snapshot().work }

    /// No mutable run, output prefix, sampler or policy is exposed between steps.
    pub fn progress(&self) -> NativeEvaluationProgress {
        let snapshot = self.snapshot();
        NativeEvaluationProgress { status: self.status, work: snapshot.work,
            tokenization: snapshot.tokenization, admission_steps: self.admission_steps,
            requested_prompt_tokens: snapshot.requested, reviewed_prompt_tokens: snapshot.reviewed,
            released_answer_tokens: snapshot.released, finish: snapshot.finish }
    }

    /// Freeze ALL original request bytes and admit the entire prompt/output and
    /// numerical request before any inference. No framing, salt, policy text or
    /// hidden prefix is appended. Refusal consumes the sole evaluation opportunity.
    /// Successful admission yields Running at position zero, not a judgment.
    pub fn begin(&mut self, input: &WorkerInput) -> Result<NativeEvaluationProgress, NativeEvaluationError> {
        self.begin_with(input, false)
    }

    /// Freeze the SAME complete request bytes and start original prompt
    /// admission without draining its tokenizer. Subsequent advances each run
    /// at most one original tokenizer step until the full prompt is admitted.
    /// The final admission step yields Running with zero numerical work; the
    /// next advance may compute the first token. No input is rendered or added.
    /// Initial bounded copies, allocation and control/segmentation scans remain
    /// synchronous; this API does not preempt those operations.
    pub fn begin_cooperative(&mut self, input: &WorkerInput) -> Result<NativeEvaluationProgress, NativeEvaluationError> {
        self.begin_with(input, true)
    }

    fn begin_with(&mut self, input: &WorkerInput, cooperative: bool)
        -> Result<NativeEvaluationProgress, NativeEvaluationError>
    {
        if self.status != NativeEvaluationStatus::AwaitingInput { return Err(Error::WrongState.into()); }
        self.status = NativeEvaluationStatus::Evaluating;
        self.input = Some(input.clone());
        let result = self.begin_once(input, cooperative);
        self.status = match result {
            Ok(status) => status,
            Err(error) => NativeEvaluationStatus::Failed(error),
        };
        result.map(|_| self.progress())
    }

    fn begin_once(&mut self, input: &WorkerInput, cooperative: bool)
        -> Result<NativeEvaluationStatus, NativeEvaluationError>
    {
        let actual = input.actual_input();
        if actual.input_profile() != &self.policy.input_profile { return Err(Error::Binding.into()); }
        let request = TextGenerationRequest {
            prompt: actual.submitted_bytes().to_vec(), prefix_controls: Vec::new(),
            max_new_tokens: self.policy.max_new_tokens, stop_tokens: self.policy.stop_tokens.clone(),
            tokenization: self.policy.tokenization, generation: self.policy.generation,
            max_output_bytes: self.policy.max_output_bytes,
        };
        let mut snapshot = self.snapshot();
        let Execution::Decoder(decoder) = std::mem::replace(&mut self.execution, Execution::Closed(snapshot)) else {
            return Err(Error::WrongState.into());
        };
        let result = if cooperative {
            (*decoder).into_generation_admission(0, request)
                .map(|admission| Execution::Admitting(Box::new(admission)))
        } else {
            (*decoder).into_generation(0, request)
                .map(|run| Execution::Running(Box::new(run)))
        };
        match result {
            Ok(execution) => self.execution = execution,
            Err(error) => {
                // Admission consumed its owner. Preserve its actual tokenizer
                // work even when no generation session/report could be made.
                snapshot.tokenization = error.tokenization;
                self.execution = Execution::Closed(snapshot);
                return Err(NativeEvaluationError::Admission(error));
            }
        }
        Ok(if cooperative { NativeEvaluationStatus::Admitting } else { NativeEvaluationStatus::Running })
    }

    /// During Admitting, perform at most ONE original tokenizer step and yield
    /// before numerical work even when admission completes. During Running,
    /// compute at most ONE original forced/sampled token. Numerical holds,
    /// exhausted budgets and invalid complete answers remain explicit failures.
    /// No bytes or vote are returned before the terminal control's own review.
    /// Stale positions do not spend work. A terminal judgment can be inspected
    /// repeatedly without another draw; evaluate/begin never accept a second input.
    pub fn advance(&mut self, expected_position: u64) -> Result<NativeEvaluationProgress, NativeEvaluationError> {
        self.advance_with(expected_position, || {})
    }

    // Private causal interruption seam, not a public callback or token source.
    fn advance_with<F: FnOnce()>(&mut self, expected_position: u64, after_native: F)
        -> Result<NativeEvaluationProgress, NativeEvaluationError>
    {
        match self.status {
            NativeEvaluationStatus::Admitting | NativeEvaluationStatus::Running | NativeEvaluationStatus::Judged(_) => {}
            NativeEvaluationStatus::Failed(error) => return Err(error),
            _ => return Err(Error::WrongState.into()),
        }
        if expected_position != self.position() { return Err(Error::Stale.into()); }
        if matches!(self.status, NativeEvaluationStatus::Judged(_)) { return Ok(self.progress()); }
        // If execution or report handling unwinds, a runnable state is never restored.
        self.status = NativeEvaluationStatus::Evaluating;
        let result = self.advance_once(expected_position, after_native);
        if let Err(error) = result { self.status = NativeEvaluationStatus::Failed(error); }
        result.map(|()| self.progress())
    }

    fn advance_once<F: FnOnce()>(&mut self, expected_position: u64, after_native: F)
        -> Result<(), NativeEvaluationError>
    {
        if matches!(&self.execution, Execution::Admitting(_)) {
            return self.advance_admission_once(after_native);
        }
        let Execution::Running(run) = &mut self.execution else { return Err(Error::WrongState.into()); };
        run.advance(expected_position)?;
        after_native();
        if run.progress().finish().is_none() {
            self.status = NativeEvaluationStatus::Running;
            return Ok(());
        }
        let snapshot = self.snapshot();
        let Execution::Running(run) = std::mem::replace(&mut self.execution, Execution::Closed(snapshot)) else {
            return Err(Error::WrongState.into());
        };
        let (decoder, report) = (*run).into_parts()?;
        self.execution = Execution::Decoder(Box::new(decoder));
        let verdict = self.finish_report(report)?;
        self.status = NativeEvaluationStatus::Judged(verdict);
        Ok(())
    }

    fn advance_admission_once<F: FnOnce()>(&mut self, after_step: F)
        -> Result<(), NativeEvaluationError>
    {
        let Execution::Admitting(admission) = &mut self.execution else { return Err(Error::WrongState.into()); };
        self.admission_steps = self.admission_steps.checked_add(1).ok_or(Error::Overflow)?;
        let progress = admission.advance().map_err(NativeEvaluationError::Admission)?;
        after_step();
        if progress == TokenizationProgress::Pending {
            self.status = NativeEvaluationStatus::Admitting;
            return Ok(());
        }
        let snapshot = self.snapshot();
        let Execution::Admitting(admission) = std::mem::replace(&mut self.execution, Execution::Closed(snapshot)) else {
            return Err(Error::WrongState.into());
        };
        // The ORIGINAL handoff checks the entire prompt/output/numerical request
        // and moves the completed encoding. It neither re-tokenizes nor infers.
        self.execution = Execution::Running(Box::new((*admission).finish_incremental()
            .map_err(NativeEvaluationError::Admission)?));
        self.status = NativeEvaluationStatus::Running;
        Ok(())
    }

    /// Destroy numerical ownership now, without computing another token. The
    /// original work/coverage remains inspectable, and live observation handles
    /// close before this returns. No unfinished owner or partial answer escapes.
    /// A caught interruption, failure or completed judgment is preserved, not
    /// relabeled as successful cancellation. False means already destroyed.
    pub fn cancel(&mut self) -> bool {
        if matches!(&self.execution, Execution::Closed(_)) { return false; }
        let snapshot = self.snapshot();
        if matches!(self.status, NativeEvaluationStatus::AwaitingInput | NativeEvaluationStatus::Admitting | NativeEvaluationStatus::Running) {
            self.status = NativeEvaluationStatus::Cancelled;
        }
        let previous = std::mem::replace(&mut self.execution, Execution::Closed(snapshot));
        drop(previous);
        true
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod cooperative_tests;
