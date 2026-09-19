//! Yield between original monitored token operations without changing the request.
//! This cursor is also the reducer for one-shot and durable generation. There is
//! no saved-cursor import, public owner trait or alternate sampling implementation.
use super::{GenerationFinish, GenerationOwner, GenerationReport, GenerationRequest,
    GenerationWork, MonitoredSampledDecoder, MonitoredStep, MonitoringStatus,
    DecoderBudget, DecoderWork, SampleBudget, MAX_DECODER_PRODUCTS,
    MAX_GENERATION_TOKENS, MAX_SAMPLING_ENTRIES, MAX_STOP_TOKENS};
use crate::action::consequence::activation::monitor::decoder::{DecoderReview, MonitoringWork};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::SamplingBudget;
use crate::Error;
use std::collections::BTreeSet;
use std::fmt;

/// A partial observation is deliberately NOT a completed GenerationReport.
/// finish() is None until the original request terminates. Previously reviewed
/// tokens remain observations even if a later token holds or computation fails.
#[derive(Clone)]
pub struct GenerationProgress {
    complete: bool,
    report: GenerationReport,
}
impl GenerationProgress {
    pub fn start_position(&self) -> u64 { self.report.start_position() }
    pub fn position(&self) -> u64 { self.report.end_position() }
    pub fn requested_prompt_tokens(&self) -> usize { self.report.requested_prompt_tokens() }
    pub fn reviewed_prompt_tokens(&self) -> usize { self.report.reviewed_prompt_tokens() }
    pub fn tokens(&self) -> &[u32] { self.report.tokens() }
    pub fn work(&self) -> GenerationWork { self.report.work() }
    pub fn last_review(&self) -> Option<&DecoderReview> { self.report.last_review() }
    pub fn finish(&self) -> Option<GenerationFinish> { self.complete.then_some(self.report.finish()) }
    pub fn report(&self) -> Option<&GenerationReport> { self.complete.then_some(&self.report) }
}
impl fmt::Debug for GenerationProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenerationProgress").field("position", &self.position())
            .field("reviewed_prompt_tokens", &self.reviewed_prompt_tokens())
            .field("released_tokens", &self.tokens().len()).field("finish", &self.finish())
            .field("work", &self.work()).finish_non_exhaustive()
    }
}

/// Owns the numerical decoder, rather than lending a bypassable mutable borrow.
/// Dropping or forgetting this object cannot return an unfinished decoder to the
/// caller. The original model may independently exist elsewhere; this is not OS
/// isolation. A completed held/failed owner keeps its original terminal latch.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::incremental::GenerationSession;
/// fn bypass(run: &mut GenerationSession) { run.decoder_mut(); }
/// ```
pub struct GenerationSession {
    owner: MonitoredSampledDecoder,
    cursor: GenerationCursor,
    interrupted: bool,
}
impl fmt::Debug for GenerationSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenerationSession").field("progress", &self.progress())
            .field("interrupted", &self.interrupted).finish_non_exhaustive()
    }
}
impl MonitoredSampledDecoder {
    /// Consume this sole numerical owner and freeze the complete request before
    /// its first token. Admission errors consume the supplied owner; use generate
    /// for the existing borrowed one-shot API. No computation occurs on refusal.
    pub fn into_generation(self, expected_position: u64, request: GenerationRequest)
        -> Result<GenerationSession, Error>
    {
        let cursor = GenerationCursor::new(&self, expected_position, request)?;
        Ok(GenerationSession { owner: self, cursor, interrupted: false })
    }
}
impl GenerationSession {
    pub fn progress(&self) -> GenerationProgress { self.cursor.progress() }
    pub fn position(&self) -> u64 { self.cursor.position() }
    pub fn sampled_draws(&self) -> u64 { self.owner.sampled_draws() }
    pub fn decoder_work(&self) -> DecoderWork { self.owner.decoder_work() }
    pub fn monitoring_work(&self) -> MonitoringWork { self.owner.monitoring_work() }

    /// Compute at most one original token. Stale positions refuse without work.
    /// Polling a terminal session is idempotent. A stop token is checked only
    /// after its own review, and yields no output token on this residual lane.
    pub fn advance(&mut self, expected_position: u64) -> Result<GenerationProgress, Error> {
        if self.interrupted { return Err(Error::WrongState); }
        if expected_position != self.position() { return Err(Error::Stale); }
        // Set before entering any operation that can unwind. Forgetting the
        // session cannot bypass this either, since it OWNS the decoder.
        self.interrupted = true;
        self.cursor.advance(&mut self.owner)?;
        self.interrupted = false;
        Ok(self.progress())
    }
    pub fn run_to_stop(&mut self) -> Result<GenerationProgress, Error> {
        if self.interrupted { return Err(Error::WrongState); }
        while !self.cursor.is_complete() {
            self.interrupted = true;
            self.cursor.advance(&mut self.owner)?;
            self.interrupted = false;
        }
        Ok(self.progress())
    }
    /// No abandonment accessor: only an actually terminated request can return
    /// its numerical owner. Held/failed original owners still cannot advance.
    pub fn into_parts(self) -> Result<(MonitoredSampledDecoder, GenerationReport), Error> {
        if self.interrupted || !self.cursor.is_complete() { return Err(Error::Incomplete); }
        Ok((self.owner, self.cursor.into_report()?))
    }
}

/// Reconstructed by replaying ORIGINAL operations, never decoded from bytes.
/// The owning durable layer fences access between calls and binds the actual
/// model/actor predecessor; this cursor conserves only numerical request work.
pub(crate) struct GenerationCursor {
    request: GenerationRequest,
    stops: BTreeSet<u32>,
    vocabulary: usize,
    count: usize,
    index: usize,
    complete: bool,
    report: GenerationReport,
}
impl GenerationCursor {
    pub(crate) fn new<O: GenerationOwner>(owner: &O, expected_position: u64,
        request: GenerationRequest) -> Result<Self, Error>
    {
        if owner.generation_status()? != MonitoringStatus::Ready { return Err(Error::WrongState); }
        if owner.generation_position()? != expected_position { return Err(Error::Stale); }
        let shape = owner.generation_profile()?.shape();
        let vocabulary = shape.vocabulary;
        let count = request.prompt.len().checked_add(request.max_new_tokens).ok_or(Error::Overflow)?;
        if count > MAX_GENERATION_TOKENS || request.stop_tokens.len() > MAX_STOP_TOKENS
            || request.budget.scalar_products > MAX_DECODER_PRODUCTS
            || request.budget.sampling_entries > MAX_SAMPLING_ENTRIES
        { return Err(Error::Limit); }
        if count == 0 || (request.prompt.is_empty() && expected_position == 0)
            || request.prompt.iter().chain(&request.stop_tokens).any(|token| *token as usize >= vocabulary)
        { return Err(Error::InvalidInput); }
        let start = usize::try_from(expected_position).map_err(|_| Error::Limit)?;
        if start.checked_add(count).ok_or(Error::Overflow)? > shape.context { return Err(Error::Limit); }
        let stops: BTreeSet<u32> = request.stop_tokens.iter().copied().collect();
        if stops.len() != request.stop_tokens.len() { return Err(Error::Duplicate); }
        let prompt_products = owner.generation_estimate(request.prompt.len())?.scalar_products()?;
        if prompt_products > request.budget.scalar_products { return Err(Error::Limit); }
        let mut tokens = Vec::new();
        tokens.try_reserve_exact(request.max_new_tokens).map_err(|_| Error::Limit)?;
        let report = GenerationReport {
            start_position: expected_position, end_position: expected_position,
            requested_prompt_tokens: request.prompt.len(), reviewed_prompt_tokens: 0,
            tokens, finish: GenerationFinish::TokenLimit, work: GenerationWork::default(), last_review: None,
        };
        Ok(Self { request, stops, vocabulary, count, index: 0, complete: false, report })
    }
    pub(crate) fn is_complete(&self) -> bool { self.complete }
    pub(crate) fn position(&self) -> u64 { self.report.end_position }
    pub(crate) fn progress(&self) -> GenerationProgress {
        GenerationProgress { complete: self.complete, report: self.report.clone() }
    }
    pub(crate) fn into_report(self) -> Result<GenerationReport, Error> {
        if !self.complete { return Err(Error::Incomplete); }
        Ok(self.report)
    }
    pub(crate) fn advance<O: GenerationOwner>(&mut self, owner: &mut O) -> Result<(), Error> {
        if self.complete { return Ok(()); }
        let position = owner.generation_position()?;
        if position != self.report.end_position { return Err(Error::Stale); }
        let products = match owner.generation_estimate(1).and_then(DecoderWork::scalar_products) {
            Ok(products) => products,
            Err(error) => { self.finish(GenerationFinish::Failed(error)); self.report.last_review = None; return Ok(()); }
        };
        let sampled = self.index >= self.request.prompt.len();
        let sampling = if sampled { self.vocabulary as u64 } else { 0 };
        if products > self.request.budget.scalar_products - self.report.work.admitted_scalar_products
            || sampling > self.request.budget.sampling_entries - self.report.work.admitted_sampling_entries
        { self.finish(GenerationFinish::BudgetExhausted); return Ok(()); }
        // Preserve the original compound preflight and single admission charge.
        self.report.work.admitted_scalar_products += products;
        self.report.work.admitted_sampling_entries += sampling;
        if sampled { self.report.work.attempted_samples += 1; }
        let budget = DecoderBudget { scalar_products: products };
        let result = if sampled {
            owner.generation_sampled(position, SampleBudget {
                decoder: budget, sampling: SamplingBudget { vocabulary: self.vocabulary },
            })
        } else { owner.generation_forced(position, self.request.prompt[self.index], budget) };
        self.report.end_position = owner.generation_position()?;
        self.index += 1;
        match result {
            Err(error) => {
                self.report.last_review = None;
                self.finish(GenerationFinish::Failed(error));
            }
            Ok(MonitoredStep::Held(review)) => {
                self.report.last_review = Some(review);
                self.finish(GenerationFinish::Held);
            }
            Ok(MonitoredStep::Released(step)) => {
                let token = step.step().token;
                self.report.last_review = Some(step.review);
                if sampled {
                    if self.stops.contains(&token) { self.finish(GenerationFinish::StopToken); return Ok(()); }
                    self.report.tokens.push(token);
                } else { self.report.reviewed_prompt_tokens += 1; }
                if self.index == self.count { self.finish(GenerationFinish::TokenLimit); }
            }
        }
        Ok(())
    }
    fn finish(&mut self, finish: GenerationFinish) {
        self.report.finish = finish;
        self.complete = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{GenerationBudget, GenerationRequest};
    use super::super::super::tests::fixture;

    fn request(prompt: &[u32], new: usize) -> GenerationRequest {
        GenerationRequest { prompt: prompt.to_vec(), max_new_tokens: new, stop_tokens: Vec::new(),
            budget: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES } }
    }

    #[test]
    fn segmentation_matches_original_sampler_and_does_not_call_partial_progress_complete() {
        let (run, budget) = fixture();
        let (mut reference, _) = fixture();
        let mut session = run.into_generation(0, request(&[0, 1], 2)).unwrap();
        assert!(session.progress().report().is_none());
        for (position, token) in [(0, 0), (1, 1)] {
            reference.advance_forced(position, token, budget.decoder).unwrap();
            let p = session.advance(position).unwrap();
            assert_eq!(p.position(), position + 1);
            assert_eq!(p.reviewed_prompt_tokens(), position as usize + 1);
            assert!(p.finish().is_none()); assert!(p.report().is_none());
            assert!(p.tokens().is_empty());
        }
        let mut tokens = Vec::new();
        for position in 2..4 {
            let MonitoredStep::Released(step) = reference.advance_sampled(position, budget).unwrap().into_monitored()
                else { panic!("quiet reference"); };
            tokens.push(step.step().token);
            let p = session.advance(position).unwrap();
            assert_eq!(p.tokens(), tokens.as_slice());
            assert_eq!(p.finish(), (position == 3).then_some(GenerationFinish::TokenLimit));
        }
        assert_eq!(session.sampled_draws(), 2);
        assert_eq!(session.decoder_work(), reference.decoder_work());
        assert_eq!(session.monitoring_work(), reference.monitoring_work());
        let (owner, report) = session.into_parts().unwrap();
        assert_eq!(owner.position(), 4); assert_eq!(report.tokens(), tokens.as_slice());
    }

    #[test]
    fn stale_steps_and_terminal_polling_neither_spend_work_nor_reroll() {
        let (run, _) = fixture();
        let mut session = run.into_generation(0, request(&[0], 1)).unwrap();
        assert_eq!(session.advance(1).unwrap_err(), Error::Stale);
        assert_eq!(session.position(), 0); assert_eq!(session.decoder_work().tokens, 0);
        session.advance(0).unwrap();
        let before = session.decoder_work();
        assert_eq!(session.advance(0).unwrap_err(), Error::Stale);
        assert_eq!(session.decoder_work(), before);
        let final_progress = session.advance(1).unwrap();
        let before = session.decoder_work();
        for _ in 0..3 {
            let p = session.advance(2).unwrap();
            assert_eq!(p.tokens(), final_progress.tokens());
            assert_eq!(p.work(), final_progress.work());
            assert_eq!(p.finish(), Some(GenerationFinish::TokenLimit));
        }
        assert_eq!(session.decoder_work(), before); assert_eq!(session.sampled_draws(), 1);
    }

    #[test]
    fn full_prompt_preflight_and_cumulative_budget_survive_yield_boundaries() {
        for products in [75, 76, 120] {
            let (run, _) = fixture();
            let mut r = request(&[0, 1], 2); r.budget.scalar_products = products;
            let session = run.into_generation(0, r);
            if products == 75 { assert_eq!(session.unwrap_err(), Error::Limit); continue; }
            let mut session = session.unwrap();
            session.advance(0).unwrap(); session.advance(1).unwrap();
            assert_eq!(session.progress().work().admitted_scalar_products, 76);
            let p = session.advance(2).unwrap();
            if products == 76 {
                assert_eq!(p.finish(), Some(GenerationFinish::BudgetExhausted));
                assert_eq!(p.position(), 2); assert_eq!(session.sampled_draws(), 0);
            } else {
                assert_eq!(p.work().admitted_scalar_products, 120); assert!(p.finish().is_none());
                let p = session.advance(3).unwrap();
                assert_eq!(p.finish(), Some(GenerationFinish::BudgetExhausted));
                assert_eq!(p.position(), 3); assert_eq!(session.sampled_draws(), 1);
            }
        }
    }

    #[test]
    fn stops_in_prompt_do_not_stop_and_a_generated_stop_is_reviewed_then_suppressed() {
        let (run, _) = fixture();
        let mut r = request(&[0, 1], 2); r.stop_tokens = vec![0, 1];
        let mut session = run.into_generation(0, r).unwrap();
        assert!(session.advance(0).unwrap().finish().is_none());
        assert!(session.advance(1).unwrap().finish().is_none());
        let p = session.advance(2).unwrap();
        assert_eq!(p.finish(), Some(GenerationFinish::StopToken)); assert!(p.tokens().is_empty());
        assert_eq!(p.reviewed_prompt_tokens(), 2); assert_eq!(session.sampled_draws(), 1);
        assert_eq!(session.monitoring_work().frame_reviews, 3);
    }

    #[test]
    fn held_candidate_is_not_an_eos_and_retains_its_draw() {
        let (mut run, budget) = fixture();
        run.advance_forced(0, 0, budget.decoder).unwrap();
        let work = run.monitoring_work();
        run.monitored.budget.encoded_bytes = work.encoded_bytes;
        run.monitored.budget.probe_coordinates = work.probe_coordinates;
        let mut r = request(&[], 2); r.stop_tokens = vec![0, 1];
        let mut session = run.into_generation(1, r).unwrap();
        let p = session.advance(1).unwrap();
        assert_eq!(p.finish(), Some(GenerationFinish::Held)); assert!(p.tokens().is_empty());
        assert_eq!(p.position(), 2); assert_eq!(session.sampled_draws(), 1);
        session.advance(2).unwrap(); assert_eq!(session.sampled_draws(), 1);
        let (mut run, _) = session.into_parts().unwrap();
        assert_eq!(run.status(), MonitoringStatus::Held);
        assert!(matches!(run.advance_sampled(2, budget), Err(Error::WrongState)));
    }

    #[test]
    fn failed_postcompute_review_returns_no_token_and_no_old_quiet_review() {
        let (mut run, budget) = fixture();
        run.advance_forced(0, 0, budget.decoder).unwrap();
        run.monitored.work.frame_reviews = u64::MAX;
        let mut session = run.into_generation(1, request(&[], 2)).unwrap();
        let p = session.advance(1).unwrap();
        assert_eq!(p.finish(), Some(GenerationFinish::Failed(Error::Overflow)));
        assert!(p.tokens().is_empty()); assert!(p.last_review().is_none());
        assert_eq!(p.position(), 2); assert_eq!(session.sampled_draws(), 1);
        let (run, _) = session.into_parts().unwrap();
        assert_eq!(run.status(), MonitoringStatus::Failed(Error::Overflow));
    }

    #[test]
    fn an_unfinished_request_cannot_return_an_owner_for_a_different_prompt() {
        let (run, _) = fixture();
        let mut session = run.into_generation(0, request(&[0, 1], 1)).unwrap();
        session.advance(0).unwrap();
        assert!(matches!(session.into_parts(), Err(Error::Incomplete)));
        let (run, _) = fixture();
        let mut session = run.into_generation(0, request(&[0, 1], 0)).unwrap();
        session.run_to_stop().unwrap();
        let (mut run, report) = session.into_parts().unwrap();
        assert_eq!(report.reviewed_prompt_tokens(), 2);
        assert_eq!(run.generate(2, request(&[], 1)).unwrap().tokens().len(), 1);
    }
}
