//! Explicit bounded experiment inputs. Parsing does not run inference or choose seeds.
use super::{PairedRolloutSuite, RolloutBudget, RolloutBuildError, RolloutCase,
    RolloutCriteria, RolloutProtocol, RolloutReport, RolloutWork, MAX_ROLLOUT_CASES,
    MAX_ROLLOUT_TOKENS, MAX_EFFECT_PATTERNS, MAX_EFFECT_PATTERN_TOKENS};
use crate::action::consequence::activation::probe::training::CaseOrigin;
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderModel};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::SamplingPolicy;
use crate::strict_json::{self, ErrorKind, Json, Limits};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const MAX_ROLLOUT_PLAN_BYTES: usize = 1_048_576;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RolloutPlanError { Syntax, Limit, Field(&'static str), Contract(Error) }
impl From<Error> for RolloutPlanError { fn from(value: Error) -> Self { Self::Contract(value) } }
impl fmt::Display for RolloutPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for RolloutPlanError {}

/// Immutable parsed specification, not a session or proof of held-out independence.
#[derive(Clone, Debug)]
pub struct RolloutPlan {
    identity: DecoderIdentity,
    context: usize,
    protocol: RolloutProtocol,
    cases: Vec<RolloutCase>,
    criteria: RolloutCriteria,
    budget: RolloutWork,
}
impl RolloutPlan {
    pub fn decode(bytes: &[u8]) -> Result<Self, RolloutPlanError> {
        let value = strict_json::parse(bytes, Limits { max_bytes: MAX_ROLLOUT_PLAN_BYTES,
            max_depth: 6, max_items: 131_072, max_string_bytes: 128 })
            .map_err(|error| match error.kind {
                ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit => RolloutPlanError::Limit,
                _ => RolloutPlanError::Syntax,
            })?;
        let root = object(&value, &["schema", "identity", "context", "sampling", "max_new_tokens",
            "stop_tokens", "effect_patterns", "cases", "criteria", "budget"], "$")?;
        if root["schema"].as_str() != Some("fa.sampled-rollout/1") { return Err(RolloutPlanError::Field("schema")); }
        let id = object(&root["identity"], &["tenant", "model", "model_generation", "tokenizer_generation", "profile_generation"], "identity")?;
        let identity = DecoderIdentity { tenant: positive(&id["tenant"], "tenant")?, model: positive(&id["model"], "model")?,
            model_generation: positive(&id["model_generation"], "model_generation")?,
            tokenizer_generation: positive(&id["tokenizer_generation"], "tokenizer_generation")?,
            profile_generation: positive(&id["profile_generation"], "profile_generation")? };
        let context = count(&root["context"], "context")?;
        if context == 0 || context > MAX_ROLLOUT_TOKENS { return Err(RolloutPlanError::Field("context")); }
        let sampling = object(&root["sampling"], &["id", "generation", "vocabulary", "temperature", "top_k", "top_p"], "sampling")?;
        let sampling = SamplingPolicy::new(positive(&sampling["id"], "sampling.id")?,
            positive(&sampling["generation"], "sampling.generation")?, count(&sampling["vocabulary"], "vocabulary")?,
            scalar(&sampling["temperature"], "temperature")?, count(&sampling["top_k"], "top_k")?,
            scalar(&sampling["top_p"], "top_p")?)?;
        let max_new_tokens = count(&root["max_new_tokens"], "max_new_tokens")?;
        if max_new_tokens == 0 || max_new_tokens >= context { return Err(RolloutPlanError::Field("max_new_tokens")); }
        let mut stop_tokens = BTreeSet::new();
        for token in tokens(&root["stop_tokens"], sampling.vocabulary(), sampling.vocabulary(), "stop_tokens")? {
            if !stop_tokens.insert(token) { return Err(Error::Duplicate.into()); }
        }
        let patterns = array(&root["effect_patterns"], MAX_EFFECT_PATTERNS, "effect_patterns")?;
        if patterns.is_empty() { return Err(Error::InvalidInput.into()); }
        let mut unique_patterns = BTreeSet::new();
        let mut effect_patterns = Vec::new();
        for pattern in patterns {
            let pattern = tokens(pattern, sampling.vocabulary(), MAX_EFFECT_PATTERN_TOKENS, "effect_patterns")?;
            if pattern.is_empty() { return Err(Error::InvalidInput.into()); }
            if !unique_patterns.insert(pattern.clone()) { return Err(Error::Duplicate.into()); }
            effect_patterns.push(pattern);
        }
        let mut cases = Vec::new();
        let mut tasks = BTreeSet::new(); let mut lineages = BTreeSet::new();
        let mut prompts = BTreeSet::new(); let mut streams = BTreeSet::new();
        let mut total = 0_usize;
        for case in array(&root["cases"], MAX_ROLLOUT_CASES, "cases")? {
            let case = object(case, &["task", "lineage", "prompt", "random_stream", "seed"], "case")?;
            let origin = CaseOrigin { task: positive(&case["task"], "task")?, lineage: positive(&case["lineage"], "lineage")? };
            let prompt = tokens(&case["prompt"], sampling.vocabulary(), context, "prompt")?;
            let random_stream = positive(&case["random_stream"], "random_stream")?;
            if prompt.is_empty() { return Err(Error::InvalidInput.into()); }
            if !tasks.insert(origin.task) || !lineages.insert(origin.lineage) || !prompts.insert(prompt.clone())
                || !streams.insert(random_stream) { return Err(Error::Duplicate.into()); }
            let n = prompt.len().checked_add(max_new_tokens).ok_or(Error::Overflow)?;
            total = total.checked_add(n).ok_or(Error::Overflow)?;
            if n > context || total > MAX_ROLLOUT_TOKENS { return Err(Error::Limit.into()); }
            cases.push(RolloutCase { origin, prompt, random_stream, seed: integer(&case["seed"], "seed")? });
        }
        let c = object(&root["criteria"], &["minimum_benign", "minimum_effects", "minimum_timely_alarms",
            "maximum_benign_stops", "maximum_misses"], "criteria")?;
        let criteria = RolloutCriteria { minimum_benign: count(&c["minimum_benign"], "minimum_benign")?,
            minimum_effects: count(&c["minimum_effects"], "minimum_effects")?,
            minimum_timely_alarms: count(&c["minimum_timely_alarms"], "minimum_timely_alarms")?,
            maximum_benign_stops: count(&c["maximum_benign_stops"], "maximum_benign_stops")?,
            maximum_misses: count(&c["maximum_misses"], "maximum_misses")? };
        criteria.check()?;
        if criteria.minimum_benign + criteria.minimum_effects > cases.len()
            || criteria.minimum_timely_alarms > cases.len() { return Err(Error::Incomplete.into()); }
        let names = ["cases", "token_steps", "scalar_products", "sampling_entries", "comparison_entries",
            "oracle_comparisons", "monitor_bytes", "probe_coordinates", "retained_score_words"];
        let b = object(&root["budget"], &names, "budget")?;
        let mut fields = [0_u64; 9];
        for (slot, name) in fields.iter_mut().zip(names) { *slot = integer(&b[name], "budget")?; }
        let budget = RolloutWork::from_fields(fields); budget.check()?;
        Ok(Self { identity, context, protocol: RolloutProtocol { sampling, max_new_tokens, stop_tokens, effect_patterns },
            cases, criteria, budget })
    }
    pub fn identity(&self) -> DecoderIdentity { self.identity }
    pub fn context(&self) -> usize { self.context }
    pub fn protocol(&self) -> &RolloutProtocol { &self.protocol }
    pub fn cases(&self) -> &[RolloutCase] { &self.cases }
    pub fn criteria(&self) -> RolloutCriteria { self.criteria }
    pub fn budget(&self) -> RolloutWork { self.budget }

    /// Validate the exact numerical profile and complete workload before any
    /// inference. The prepared owner retains a single-use suite and its budget.
    pub fn bind(&self, model: DecoderModel, monitor_json: &[u8]) -> Result<PreparedRollout, RolloutBuildError> {
        if model.profile().identity() != self.identity || model.profile().shape().context != self.context {
            return Err(Error::Binding.into());
        }
        let suite = PairedRolloutSuite::new(model, monitor_json, self.protocol.clone(), self.cases.clone(), self.criteria)?;
        if suite.planned_work().fields().into_iter().zip(self.budget.fields()).any(|(need, have)| need > have) {
            return Err(Error::Limit.into());
        }
        Ok(PreparedRollout { suite, budget: RolloutBudget::new(self.budget)? })
    }
}
#[derive(Debug)]
pub struct PreparedRollout { suite: PairedRolloutSuite, budget: RolloutBudget }
impl PreparedRollout {
    pub fn planned_work(&self) -> RolloutWork { self.suite.planned_work() }
    pub fn remaining_budget(&self) -> RolloutWork { self.budget.remaining() }
    pub fn started(&self) -> bool { self.suite.started() }
    pub fn run(&mut self) -> Result<RolloutReport, Error> { self.suite.run(&mut self.budget) }
}
fn object<'a>(value: &'a Json, names: &[&str], field: &'static str) -> Result<&'a BTreeMap<String, Json>, RolloutPlanError> {
    let object = value.as_object().ok_or(RolloutPlanError::Field(field))?;
    if object.len() != names.len() || names.iter().any(|name| !object.contains_key(*name)) { return Err(RolloutPlanError::Field(field)); }
    Ok(object)
}
fn array<'a>(value: &'a Json, max: usize, field: &'static str) -> Result<&'a [Json], RolloutPlanError> {
    let array = value.as_array().ok_or(RolloutPlanError::Field(field))?;
    if array.len() > max { return Err(RolloutPlanError::Limit); } Ok(array)
}
fn integer(value: &Json, field: &'static str) -> Result<u64, RolloutPlanError> { value.as_u64().ok_or(RolloutPlanError::Field(field)) }
fn positive(value: &Json, field: &'static str) -> Result<u64, RolloutPlanError> {
    let n = integer(value, field)?; if n == 0 { Err(RolloutPlanError::Field(field)) } else { Ok(n) }
}
fn count(value: &Json, field: &'static str) -> Result<usize, RolloutPlanError> {
    usize::try_from(integer(value, field)?).map_err(|_| RolloutPlanError::Limit)
}
fn scalar(value: &Json, field: &'static str) -> Result<f64, RolloutPlanError> {
    let n = match value { Json::Number(n) => n.lexeme().parse::<f64>().ok(), _ => None };
    n.filter(|n| n.is_finite()).ok_or(RolloutPlanError::Field(field))
}
fn tokens(value: &Json, vocabulary: usize, maximum: usize, field: &'static str) -> Result<Vec<u32>, RolloutPlanError> {
    array(value, maximum, field)?.iter().map(|value| {
        let n = u32::try_from(integer(value, field)?).map_err(|_| RolloutPlanError::Field(field))?;
        if n as usize >= vocabulary { Err(RolloutPlanError::Field(field)) } else { Ok(n) }
    }).collect()
}
