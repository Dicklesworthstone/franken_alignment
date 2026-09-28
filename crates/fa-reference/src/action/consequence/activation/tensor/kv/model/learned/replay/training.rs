//! Fit the original codec from original decoder computations, not supplied KV.
//! This offline owner has no effect rights. Source IDs and corpus independence
//! remain caller declarations; running a model does not authenticate its weights.
use super::{FitBudget, LearnedKvCodec, LearnedKvFitCheckpoint, LearnedKvPolicy,
    ModelKvImage, check_budget, MAX_TRAINING_SOURCES};
use super::super::{FitReport, MAX_MODEL_KV_VALUES};
use super::super::fit::MAX_TRAINING_ROWS;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderModel, DecoderSession, DecoderWork, MAX_DECODER_PRODUCTS,
};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Complete original-token document. No cache, coefficient, saved success or
/// per-step callback is accepted. Documents are sorted by origin before work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodecTrainingInput {
    pub origin: u64,
    pub stream: u64,
    pub tokens: Vec<u32>,
}

/// Whole-corpus inference and retention allowances, plus the original fitter's
/// independent cap. These are logical work/value limits, not time or peak RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodecTrainingBudget {
    pub tokens: usize,
    pub source_values: usize,
    pub decoder_products: u64,
    pub fitting: FitBudget,
}
impl Default for CodecTrainingBudget {
    fn default() -> Self {
        Self { tokens: MAX_TRAINING_ROWS, source_values: MAX_MODEL_KV_VALUES,
            decoder_products: MAX_DECODER_PRODUCTS, fitting: FitBudget::default() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecTrainingStatus { Capturing, ReadyToFit, Fitted, Cancelled, Failed(Error) }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingDocumentWork {
    pub stream: u64,
    pub declared_tokens: usize,
    /// Reserved before an attempted original token, even when that token fails.
    pub attempted_tokens: usize,
    pub reserved_decoder_products: u64,
    /// Only original computations that returned successfully are counted here.
    pub accepted: DecoderWork,
    pub captured_values: usize,
    pub complete: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CodecTrainingReport {
    pub status: CodecTrainingStatus,
    pub declared_tokens: usize,
    pub declared_values: usize,
    pub admitted_decoder_products: u64,
    pub documents: BTreeMap<u64, TrainingDocumentWork>,
    /// False after an errored/unwound token: partial numerical work can be absent
    /// from accepted counts. It does not describe fitting or allocator work.
    pub all_inference_reported: bool,
    pub fit_attempted: bool,
    /// Original fit report only. Failed fits may spend work without returning it.
    pub fit: Option<FitReport>,
}

/// Captures at most one token per advance. The final fitting invocation is the
/// original bounded, synchronous fitter; it is not preemptible between sweeps.
/// Partial runs cannot export a codec, raw cache or executable decoder session.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::model::learned::replay::training::CodecTraining;
/// fn partial(run: CodecTraining) { run.into_session(); }
/// ```
pub struct CodecTraining {
    model: DecoderModel,
    inputs: Vec<CodecTrainingInput>,
    policy: LearnedKvPolicy,
    budget: CodecTrainingBudget,
    current: usize,
    session: Option<DecoderSession>,
    images: BTreeMap<u64, ModelKvImage>,
    revision: u64,
    report: CodecTrainingReport,
    fitted: Option<(LearnedKvCodec, LearnedKvFitCheckpoint)>,
}
impl fmt::Debug for CodecTraining {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodecTraining").field("revision", &self.revision)
            .field("status", &self.status()).field("document", &self.current)
            .finish_non_exhaustive()
    }
}

impl DecoderModel {
    /// Admit all documents, source IDs, context/vocabulary bounds and the SUM of
    /// their original inference work before constructing a numerical session.
    /// Each document starts with an empty cache; no preceding document leaks in.
    /// Fitting's parameter/scratch/work admission remains the original fitter's
    /// responsibility after capture; failure there does not refund capture work.
    pub fn begin_codec_training(&self, mut inputs: Vec<CodecTrainingInput>,
        policy: LearnedKvPolicy, budget: CodecTrainingBudget) -> Result<CodecTraining, Error>
    {
        check_budget(budget.fitting, FitBudget::default())?;
        if inputs.is_empty() { return Err(Error::InvalidInput); }
        if inputs.len() > MAX_TRAINING_SOURCES || budget.tokens > MAX_TRAINING_ROWS
            || budget.source_values > MAX_MODEL_KV_VALUES || budget.decoder_products > MAX_DECODER_PRODUCTS {
            return Err(Error::Limit);
        }
        let mut documents = BTreeMap::new();
        let mut streams = BTreeSet::new();
        let mut tokens = 0_usize;
        let mut products = 0_u64;
        for input in &inputs {
            if input.origin == 0 || input.stream == 0 || input.tokens.is_empty() {
                return Err(Error::InvalidInput);
            }
            if documents.contains_key(&input.origin) || !streams.insert(input.stream) { return Err(Error::Duplicate); }
            if input.tokens.iter().any(|token| *token as usize >= self.profile().shape().vocabulary) {
                return Err(Error::InvalidInput);
            }
            tokens = tokens.checked_add(input.tokens.len()).ok_or(Error::Overflow)?;
            products = products.checked_add(self.estimate(0, input.tokens.len())?.scalar_products()?).ok_or(Error::Overflow)?;
            if tokens > budget.tokens || products > budget.decoder_products { return Err(Error::Limit); }
            documents.insert(input.origin, TrainingDocumentWork { stream: input.stream,
                declared_tokens: input.tokens.len(), attempted_tokens: 0, reserved_decoder_products: 0,
                accepted: DecoderWork::default(), captured_values: 0, complete: false });
        }
        if tokens < 2 { return Err(Error::Incomplete); }
        let values = tokens.checked_mul(self.cache_profile().values_per_token()).ok_or(Error::Overflow)?;
        if values > budget.source_values || values > budget.fitting.source_values { return Err(Error::Limit); }
        inputs.sort_by_key(|input| input.origin);
        Ok(CodecTraining { model: self.clone(), inputs, policy, budget, current: 0,
            session: None, images: BTreeMap::new(), revision: 0, fitted: None,
            report: CodecTrainingReport { status: CodecTrainingStatus::Capturing,
                declared_tokens: tokens, declared_values: values, admitted_decoder_products: products,
                documents, all_inference_reported: true, fit_attempted: false, fit: None } })
    }
}

impl CodecTraining {
    pub fn revision(&self) -> u64 { self.revision }
    pub fn status(&self) -> CodecTrainingStatus { self.report.status }
    pub fn report(&self) -> &CodecTrainingReport { &self.report }
    pub fn current_origin(&self) -> Option<u64> { self.inputs.get(self.current).map(|input| input.origin) }

    fn active(&self, revision: u64) -> Result<(), Error> {
        if revision != self.revision { return Err(Error::Stale); }
        if !matches!(self.status(), CodecTrainingStatus::Capturing | CodecTrainingStatus::ReadyToFit) {
            return Err(Error::WrongState);
        }
        Ok(())
    }
    fn release(&mut self) {
        self.inputs.clear(); self.images.clear(); self.session = None; self.fitted = None;
    }

    /// Stale calls do nothing. Admitted failure/unwind latches the owner, so no
    /// partial token or fit can be retried with changed inputs or renewed caps.
    pub fn advance(&mut self, revision: u64) -> Result<CodecTrainingStatus, Error> {
        self.active(revision)?;
        let fitting = self.status() == CodecTrainingStatus::ReadyToFit;
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.report.status = CodecTrainingStatus::Failed(Error::Incomplete);
        let result = if fitting { self.fit() } else { self.capture_token() };
        match result {
            Ok(status) => { self.report.status = status; Ok(status) }
            Err(error) => {
                self.report.status = CodecTrainingStatus::Failed(error);
                self.release(); Err(error)
            }
        }
    }

    fn capture_token(&mut self) -> Result<CodecTrainingStatus, Error> {
        let input = self.inputs.get(self.current).ok_or(Error::Incomplete)?;
        if self.session.is_none() { self.session = Some(self.model.session(input.stream)?); }
        let session = self.session.as_mut().ok_or(Error::Incomplete)?;
        let position = session.tokens().len();
        let planned = self.model.estimate(position, 1)?.scalar_products()?;
        let record = self.report.documents.get_mut(&input.origin).ok_or(Error::Binding)?;
        record.attempted_tokens += 1;
        record.reserved_decoder_products = record.reserved_decoder_products.checked_add(planned).ok_or(Error::Overflow)?;
        self.report.all_inference_reported = false;
        session.advance(position as u64, input.tokens[position], DecoderBudget { scalar_products: planned })?;
        record.accepted = session.work();
        self.report.all_inference_reported = true;
        if session.tokens().len() != input.tokens.len() { return Ok(CodecTrainingStatus::Capturing); }
        // Snapshot ONLY at document completion, never the growing prefix on each
        // token. The original fitter retains these exact all-layer source values.
        let image = session.cache_image()?;
        record.captured_values = image.normalized_values();
        record.complete = true;
        self.images.insert(input.origin, image);
        self.session = None;
        self.current += 1;
        Ok(if self.current == self.inputs.len() { CodecTrainingStatus::ReadyToFit }
            else { CodecTrainingStatus::Capturing })
    }

    fn fit(&mut self) -> Result<CodecTrainingStatus, Error> {
        if self.images.len() != self.inputs.len() || !self.report.documents.values().all(|record| record.complete) {
            return Err(Error::Incomplete);
        }
        self.report.fit_attempted = true;
        let (codec, checkpoint) = LearnedKvCodec::fit_with_checkpoint(self.policy, &self.images, self.budget.fitting)?;
        self.report.fit = Some(codec.fit_report().clone());
        self.fitted = Some((codec, checkpoint));
        // The verified checkpoint now owns the immutable training images. Keep no
        // redundant corpus or image map; it is not a second copy of scalar data.
        self.inputs.clear(); self.images.clear();
        Ok(CodecTrainingStatus::Fitted)
    }

    /// Abandon unfinished offline work; records of attempted/completed work stay.
    pub fn cancel(&mut self, revision: u64) -> Result<(), Error> {
        self.active(revision)?;
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.report.status = CodecTrainingStatus::Cancelled;
        self.release(); Ok(())
    }

    pub fn finish(mut self) -> Result<(LearnedKvCodec, LearnedKvFitCheckpoint, CodecTrainingReport), Error> {
        match self.status() {
            CodecTrainingStatus::Fitted => {
                let (codec, checkpoint) = self.fitted.take().ok_or(Error::Incomplete)?;
                Ok((codec, checkpoint, self.report))
            }
            CodecTrainingStatus::Failed(error) => Err(error),
            CodecTrainingStatus::Cancelled => Err(Error::WrongState),
            _ => Err(Error::Incomplete),
        }
    }
}
