//! Source-level counterfactuals through the ORIGINAL learned generation engine.
//! Only an exact expected prompt span changes. No arm, capture or permit escapes.
use super::super::{DecoderModel, MAX_DECODER_PRODUCTS};
use super::super::monitoring::LearnedDecoderPolicy;
use super::super::sampling::{SampledToken, monitored::{
    GenerationBudget, GenerationPhase, GenerationSpec, GenerationStatus,
    GenerationTelemetryBudget, GenerationTelemetryWork, GenerationWork, LearnedGeneration,
    MAX_GENERATION_SCORES, MAX_GENERATION_TOKENS,
}};
use crate::action::consequence::activation::monitor::MonitorOutcome;
use crate::Error;
use std::fmt;
use std::rc::Rc;

/// Original token IDs, never text that is decoded and re-tokenized. Empty
/// expected/replacement spans allow explicit insertion/deletion; equality is a
/// sham control. The resulting prompt must still satisfy the original spec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptEdit {
    pub start: usize,
    pub expected: Vec<u32>,
    pub replacement: Vec<u32>,
}
impl PromptEdit {
    fn apply(&self, prompt: &[u32]) -> Result<Vec<u32>, Error> {
        if self.expected.len() > MAX_GENERATION_TOKENS || self.replacement.len() > MAX_GENERATION_TOKENS {
            return Err(Error::Limit);
        }
        let end = self.start.checked_add(self.expected.len()).ok_or(Error::Overflow)?;
        if prompt.get(self.start..end) != Some(self.expected.as_slice()) { return Err(Error::Binding); }
        let length = prompt.len().checked_sub(self.expected.len())
            .and_then(|n| n.checked_add(self.replacement.len())).ok_or(Error::Overflow)?;
        if length > MAX_GENERATION_TOKENS { return Err(Error::Limit); }
        let mut result = Vec::new();
        result.try_reserve_exact(length).map_err(|_| Error::Limit)?;
        result.extend_from_slice(&prompt[..self.start]);
        result.extend_from_slice(&self.replacement);
        result.extend_from_slice(&prompt[end..]);
        Ok(result)
    }
}

/// Fixed before computation. Both arms share this actual model/policy recipe,
/// original sampling seed/stream, stopping rules and separate per-arm ceilings.
#[derive(Clone, Debug)]
pub struct PromptExperimentConfig {
    pub stream: u64,
    pub evaluation_origin: u64,
    pub original: GenerationSpec,
    pub policy: LearnedDecoderPolicy,
    pub generation: GenerationBudget,
    pub telemetry: GenerationTelemetryBudget,
}

/// Extra admission for BOTH complete arms, with no early-stop discount. Per-arm
/// numerical and aggregate telemetry ceilings also remain in force. This is not
/// a global research escrow, peak-memory limit or measured physical work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptComparisonBudget {
    pub positions: usize,
    pub decoder_products: u64,
    pub vocabulary_scores: u64,
}
impl Default for PromptComparisonBudget {
    fn default() -> Self {
        Self { positions: 2 * MAX_GENERATION_TOKENS,
            decoder_products: 2 * MAX_DECODER_PRODUCTS, vocabulary_scores: 2 * MAX_GENERATION_SCORES }
    }
}
impl PromptComparisonBudget {
    fn admits(self, need: Self) -> Result<(), Error> {
        let maximum = Self::default();
        if self.positions > maximum.positions || self.decoder_products > maximum.decoder_products
            || self.vocabulary_scores > maximum.vocabulary_scores || need.positions > self.positions
            || need.decoder_products > self.decoder_products || need.vocabulary_scores > self.vocabulary_scores {
            return Err(Error::Limit);
        }
        Ok(())
    }
}

struct Plan {
    id: u64,
    model: DecoderModel,
    config: PromptExperimentConfig,
    edit: PromptEdit,
    treated: GenerationSpec,
    reservation: PromptComparisonBudget,
}

/// A frozen L7 intervention recipe, not a live actor modification or a verified
/// causal claim. Different prompt lengths also change subsequent absolute
/// positions; that consequence of the edit is explicit, not controlled away.
#[derive(Clone)]
pub struct PromptIntervention { data: Rc<Plan> }
impl fmt::Debug for PromptIntervention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PromptIntervention").field("id", &self.id())
            .field("reservation", &self.reservation()).finish_non_exhaustive()
    }
}
impl DecoderModel {
    pub fn prompt_intervention(&self, id: u64, config: PromptExperimentConfig, edit: PromptEdit)
        -> Result<PromptIntervention, Error>
    {
        if id == 0 || config.stream == 0 || config.evaluation_origin == 0 { return Err(Error::InvalidInput); }
        let prompt = edit.apply(config.original.prompt())?;
        let treated = GenerationSpec::new(prompt, config.original.max_new_tokens(),
            config.original.stop_tokens().clone(), config.original.sampling().clone())?;
        let left = self.estimate_monitored_generation(&config.original)?;
        let right = self.estimate_monitored_generation(&treated)?;
        let reservation = PromptComparisonBudget {
            positions: left.audited_positions.checked_add(right.audited_positions).ok_or(Error::Overflow)?,
            decoder_products: left.decoder.scalar_products()?.checked_add(right.decoder.scalar_products()?).ok_or(Error::Overflow)?,
            vocabulary_scores: left.vocabulary_scores.checked_add(right.vocabulary_scores).ok_or(Error::Overflow)?,
        };
        PromptComparisonBudget::default().admits(reservation)?;
        Ok(PromptIntervention { data: Rc::new(Plan { id, model: self.clone(), config, edit, treated, reservation }) })
    }
}
impl PromptIntervention {
    pub fn id(&self) -> u64 { self.data.id }
    pub fn config(&self) -> &PromptExperimentConfig { &self.data.config }
    pub fn edit(&self) -> &PromptEdit { &self.data.edit }
    pub fn treated_spec(&self) -> &GenerationSpec { &self.data.treated }
    pub fn reservation(&self) -> PromptComparisonBudget { self.data.reservation }

    /// Both original constructors must succeed before returning either arm.
    /// No supplied cache, saved quiet verdict or helper result is installed.
    pub fn begin(&self, budget: PromptComparisonBudget) -> Result<PromptComparison, Error> {
        budget.admits(self.reservation())?;
        let config = self.config();
        let start = |spec: &GenerationSpec| self.data.model.monitored_generation_with_telemetry(
            config.stream, config.evaluation_origin, spec.clone(), config.policy.clone(),
            config.generation, config.telemetry);
        let baseline = start(&config.original)?;
        let treated = start(self.treated_spec())?;
        Ok(PromptComparison { plan: self.clone(), baseline, treated,
            status: PromptComparisonStatus::Active, revision: 0, calls: [0; 2], last: None })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptArm { Baseline, Treated }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptComparisonStatus {
    Active,
    /// Both arms stopped. Inspect EACH outcome: a hold/failure is not completion
    /// of that arm's requested continuation, much less absence of a behavior.
    Stopped,
    Failed(Error),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptArmReport {
    pub status: GenerationStatus,
    pub attempted_calls: usize,
    pub accepted_prompt_tokens: usize,
    pub accepted_continuation_tokens: usize,
    pub numerical: GenerationWork,
    pub telemetry: GenerationTelemetryWork,
    /// Preparation failure/unwind may spend bounded work without a report.
    pub all_attempted_telemetry_reported: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptComparisonReport {
    pub experiment: u64,
    pub revision: u64,
    pub status: PromptComparisonStatus,
    pub reservation: PromptComparisonBudget,
    pub baseline: PromptArmReport,
    pub treated: PromptArmReport,
}

/// Experimental output only. A held token exposes neither its selected ID nor
/// random sample/logits. No original DecoderStep, SourceFrame or event is exported.
#[derive(Clone, Debug)]
pub struct PromptArmStep {
    pub position: u64,
    pub phase: GenerationPhase,
    pub status: GenerationStatus,
    pub audit: Option<MonitorOutcome>,
    pub token: Option<u32>,
    pub sample: Option<SampledToken>,
    pub logits: Option<Rc<[f32]>>,
}
#[derive(Clone, Debug)]
pub struct PromptComparisonStep {
    pub revision: u64,
    /// None means this arm was not scheduled, not that it returned a quiet audit.
    pub baseline: Option<PromptArmStep>,
    pub treated: Option<PromptArmStep>,
}

/// One immutable intervention, two private original generators. Neither arm can
/// be extracted, adopted by an effect host, retuned or retried after a hold.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::prompt::PromptComparison;
/// fn bypass(pair: &mut PromptComparison) { pair.treated_mut(); }
/// ```
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::prompt::PromptComparison;
/// use fa_reference::action::Permit;
/// fn promote(pair: PromptComparison) -> Permit { pair }
/// ```
pub struct PromptComparison {
    plan: PromptIntervention,
    baseline: LearnedGeneration,
    treated: LearnedGeneration,
    status: PromptComparisonStatus,
    revision: u64,
    calls: [usize; 2],
    last: Option<Rc<PromptComparisonStep>>,
}
impl fmt::Debug for PromptComparison {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PromptComparison").field("report", &self.report()).finish_non_exhaustive()
    }
}
impl PromptComparison {
    pub fn plan(&self) -> &PromptIntervention { &self.plan }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn status(&self) -> PromptComparisonStatus { self.status }
    pub fn last_step(&self) -> Option<&PromptComparisonStep> { self.last.as_deref() }
    fn arm(&self, arm: PromptArm) -> &LearnedGeneration {
        match arm { PromptArm::Baseline => &self.baseline, PromptArm::Treated => &self.treated }
    }
    pub fn generated_tokens(&self, arm: PromptArm) -> &[u32] { self.arm(arm).generated_tokens() }
    pub fn report(&self) -> PromptComparisonReport {
        let report = |run: &LearnedGeneration, calls| PromptArmReport {
            status: run.status(), attempted_calls: calls,
            accepted_prompt_tokens: run.accepted_tokens().len().min(run.spec().prompt().len()),
            accepted_continuation_tokens: run.generated_tokens().len(), numerical: run.work(),
            telemetry: run.telemetry_work(),
            all_attempted_telemetry_reported: !matches!(run.status(), GenerationStatus::Failed(_))
                && run.work().admitted_tokens == calls as u64,
        };
        PromptComparisonReport { experiment: self.plan.id(), revision: self.revision, status: self.status,
            reservation: self.plan.reservation(), baseline: report(&self.baseline, self.calls[0]),
            treated: report(&self.treated, self.calls[1]) }
    }

    /// Finish both prompt phases before paired continuation draws begin. Unequal
    /// prompt lengths thus pair draws by continuation ordinal, not absolute token
    /// position. A stopped arm stays stopped; its counterpart can finish normally.
    /// Ordinary generation errors remain in the report, not dropped trial rows.
    pub fn advance(&mut self, expected_revision: u64) -> Result<Rc<PromptComparisonStep>, Error> {
        if self.status != PromptComparisonStatus::Active { return Err(Error::WrongState); }
        if expected_revision != self.revision { return Err(Error::Stale); }
        let next = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        let prefill = self.baseline.status() == GenerationStatus::Prefilling
            || self.treated.status() == GenerationStatus::Prefilling;
        let scheduled = |status: GenerationStatus| if prefill {
            status == GenerationStatus::Prefilling
        } else { status.is_active() };
        let left = scheduled(self.baseline.status());
        let right = scheduled(self.treated.status());
        if !left && !right { return Err(Error::Binding); }
        self.status = PromptComparisonStatus::Failed(Error::Incomplete);
        self.revision = next;
        let baseline = if left { self.calls[0] += 1; Some(attempt(&mut self.baseline)) } else { None };
        let treated = if right { self.calls[1] += 1; Some(attempt(&mut self.treated)) } else { None };
        let step = Rc::new(PromptComparisonStep { revision: next, baseline, treated });
        self.last = Some(Rc::clone(&step));
        self.status = if self.baseline.status().is_active() || self.treated.status().is_active() {
            PromptComparisonStatus::Active
        } else { PromptComparisonStatus::Stopped };
        Ok(step)
    }
    pub fn run_to_stop(&mut self) -> Result<PromptComparisonReport, Error> {
        while self.status == PromptComparisonStatus::Active { self.advance(self.revision)?; }
        match self.status {
            PromptComparisonStatus::Failed(error) => Err(error),
            _ => Ok(self.report()),
        }
    }
}
fn attempt(run: &mut LearnedGeneration) -> PromptArmStep {
    let position = run.position();
    let phase = if run.status() == GenerationStatus::Prefilling { GenerationPhase::Prompt }
        else { GenerationPhase::Continuation };
    match run.advance(position) {
        Ok(event) => PromptArmStep { position, phase, status: event.status(), audit: Some(event.audit().outcome()),
            token: event.accepted().map(|step| step.token), sample: event.sample().cloned(),
            logits: event.accepted().map(|step| Rc::clone(&step.logits)) },
        Err(error) => PromptArmStep { position, phase, status: GenerationStatus::Failed(error),
            audit: None, token: None, sample: None, logits: None },
    }
}
