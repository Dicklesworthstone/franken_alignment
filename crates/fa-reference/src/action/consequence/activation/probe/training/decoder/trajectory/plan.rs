//! Explicit fixed trajectory evaluation, separate from fitting and calibration.

use super::{DecoderCampaign, LabelledTrajectory, MonitorExportSettings, TrajectoryBudget,
    TrajectoryCriteria, TrajectoryExpectation, TrajectoryReport, TrajectorySuite, TrajectoryWork,
    MAX_CAPTURE_TOKENS, MAX_CORPUS_CASES};
use crate::action::consequence::activation::probe::training::CaseOrigin;
use crate::action::consequence::activation::tensor::kv::{MAX_KV_POSITIONS,
    decoder::{DecoderIdentity, DecoderProfile}};
use crate::strict_json::{self, ErrorKind, Json, Limits};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

pub const MAX_TRAJECTORY_PLAN_BYTES: usize = 1_048_576;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanFileStage { Metadata, Open, Read }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrajectoryPlanError {
    Syntax,
    Limit,
    Field(String),
    Data(Error),
    NotRegular,
    Io { stage: PlanFileStage, kind: io::ErrorKind },
}
impl fmt::Display for TrajectoryPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for TrajectoryPlanError {}
impl From<Error> for TrajectoryPlanError { fn from(error: Error) -> Self { Self::Data(error) } }

/// Model identity, original-token histories, labels, effect positions, count rules
/// and global work allowances are all explicit. No monitor knobs enter this plan.
#[derive(Debug)]
pub struct TrajectoryPlan {
    identity: DecoderIdentity,
    context: usize,
    cases: Vec<LabelledTrajectory>,
    criteria: TrajectoryCriteria,
    budget: TrajectoryBudget,
}
#[derive(Debug)]
pub struct PreparedTrajectory { suite: TrajectorySuite, budget: TrajectoryBudget }
impl PreparedTrajectory {
    pub fn planned_work(&self) -> TrajectoryWork { self.suite.planned_work() }
    pub fn remaining_budget(&self) -> TrajectoryWork { self.budget.remaining() }
    pub fn started(&self) -> bool { self.suite.started() }
    pub fn run(&mut self) -> Result<TrajectoryReport, Error> { self.suite.run(&mut self.budget) }
}
impl TrajectoryPlan {
    pub fn from_json(bytes: &[u8]) -> Result<Self, TrajectoryPlanError> {
        let json = strict_json::parse(bytes, Limits { max_bytes: MAX_TRAJECTORY_PLAN_BYTES,
            max_depth: 6, max_items: MAX_CAPTURE_TOKENS + 12 * MAX_CORPUS_CASES + 128,
            max_string_bytes: 128 }).map_err(|error| match error.kind {
                ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit => TrajectoryPlanError::Limit,
                _ => TrajectoryPlanError::Syntax,
            })?;
        let root = object(&json, &["schema", "identity", "context", "criteria", "budget", "cases"], "$")?;
        if root["schema"].as_str() != Some("fa.decoder-trajectory/1") { return Err(field("schema")); }
        let ids = object(&root["identity"], &["tenant", "model", "model_generation", "tokenizer_generation", "profile_generation"], "identity")?;
        let identity = DecoderIdentity { tenant: positive(&ids["tenant"], "identity.tenant")?,
            model: positive(&ids["model"], "identity.model")?,
            model_generation: positive(&ids["model_generation"], "identity.model_generation")?,
            tokenizer_generation: positive(&ids["tokenizer_generation"], "identity.tokenizer_generation")?,
            profile_generation: positive(&ids["profile_generation"], "identity.profile_generation")? };
        let context = size(&root["context"], "context")?;
        if context == 0 || context > MAX_KV_POSITIONS { return Err(field("context")); }
        let values = object(&root["criteria"], &["id", "generation", "min_timely_alarms", "max_benign_stops", "max_missed_violations"], "criteria")?;
        let criteria = TrajectoryCriteria::new(positive(&values["id"], "criteria.id")?,
            positive(&values["generation"], "criteria.generation")?,
            size(&values["min_timely_alarms"], "criteria.min_timely_alarms")?,
            size(&values["max_benign_stops"], "criteria.max_benign_stops")?,
            size(&values["max_missed_violations"], "criteria.max_missed_violations")?)?;
        let values = object(&root["budget"], &["cases", "original_tokens", "scalar_products", "monitor_encoded_bytes", "monitor_probe_coordinates", "retained_score_words"], "budget")?;
        let budget = TrajectoryBudget::new(TrajectoryWork {
            cases: size(&values["cases"], "budget.cases")?,
            original_tokens: size(&values["original_tokens"], "budget.original_tokens")?,
            scalar_products: integer(&values["scalar_products"], "budget.scalar_products")?,
            monitor_encoded_bytes: size(&values["monitor_encoded_bytes"], "budget.monitor_encoded_bytes")?,
            monitor_probe_coordinates: size(&values["monitor_probe_coordinates"], "budget.monitor_probe_coordinates")?,
            retained_score_words: size(&values["retained_score_words"], "budget.retained_score_words")?,
        })?;
        let rows = root["cases"].as_array().ok_or_else(|| field("cases"))?;
        if rows.is_empty() || rows.len() > MAX_CORPUS_CASES { return Err(field("cases")); }
        let mut cases = Vec::new();
        cases.try_reserve_exact(rows.len()).map_err(|_| TrajectoryPlanError::Limit)?;
        let mut tasks = BTreeSet::new(); let mut lineages = BTreeSet::new();
        let mut total = 0_usize; let mut benign = 0; let mut violation = 0;
        for value in rows {
            let row = object(value, &["task", "lineage", "label", "effect_position", "tokens"], "cases[]")?;
            let origin = CaseOrigin { task: positive(&row["task"], "cases[].task")?,
                lineage: positive(&row["lineage"], "cases[].lineage")? };
            if !tasks.insert(origin.task) || !lineages.insert(origin.lineage) { return Err(Error::Duplicate.into()); }
            let tokens = row["tokens"].as_array().ok_or_else(|| field("cases[].tokens"))?;
            if tokens.is_empty() || tokens.len() > context { return Err(field("cases[].tokens")); }
            total = total.checked_add(tokens.len()).ok_or(TrajectoryPlanError::Limit)?;
            if total > MAX_CAPTURE_TOKENS { return Err(TrajectoryPlanError::Limit); }
            let tokens = tokens.iter().map(|value| {
                u32::try_from(integer(value, "cases[].tokens[]")?).map_err(|_| field("cases[].tokens[]"))
            }).collect::<Result<Vec<_>, _>>()?;
            let expectation = match row["label"].as_str() {
                Some("benign") if row["effect_position"].is_null() => { benign += 1; TrajectoryExpectation::Benign }
                Some("violation") => {
                    let effect_position = size(&row["effect_position"], "cases[].effect_position")?;
                    if effect_position >= tokens.len() { return Err(field("cases[].effect_position")); }
                    violation += 1; TrajectoryExpectation::Violation { effect_position }
                }
                _ => return Err(field("cases[].label/effect_position")),
            };
            cases.push(LabelledTrajectory { origin, expectation, tokens });
        }
        if benign == 0 || violation == 0 { return Err(Error::Incomplete.into()); }
        Ok(Self { identity, context, cases, criteria, budget })
    }

    /// Model/token admission can occur before any training work. Cross-campaign
    /// origin/history exclusion and exact monitor budgets are checked at bind.
    pub fn validate_profile(&self, profile: &DecoderProfile) -> Result<(), TrajectoryPlanError> {
        if self.identity != profile.identity() || self.context != profile.shape().context { return Err(Error::Binding.into()); }
        if self.cases.iter().flat_map(|case| &case.tokens).any(|token| *token as usize >= profile.shape().vocabulary) {
            return Err(Error::InvalidInput.into());
        }
        Ok(())
    }
    pub fn bind(self, campaign: &DecoderCampaign, settings: MonitorExportSettings) -> Result<PreparedTrajectory, TrajectoryPlanError> {
        self.validate_profile(campaign.profile())?;
        let suite = campaign.trajectory_suite(settings, self.cases, self.criteria)?;
        Ok(PreparedTrajectory { suite, budget: self.budget })
    }
}

/// Operator-controlled regular file only. Limits bound all reads, including file
/// growth; these checks do not constitute hostile-directory or TOCTOU isolation.
pub fn read_trajectory_plan(path: impl AsRef<Path>) -> Result<TrajectoryPlan, TrajectoryPlanError> {
    let fail = |stage, error: io::Error| TrajectoryPlanError::Io { stage, kind: error.kind() };
    let path = path.as_ref();
    let before = fs::symlink_metadata(path).map_err(|e| fail(PlanFileStage::Metadata, e))?;
    if !before.is_file() || before.file_type().is_symlink() { return Err(TrajectoryPlanError::NotRegular); }
    if before.len() > MAX_TRAJECTORY_PLAN_BYTES as u64 { return Err(TrajectoryPlanError::Limit); }
    let file = File::open(path).map_err(|e| fail(PlanFileStage::Open, e))?;
    let opened = file.metadata().map_err(|e| fail(PlanFileStage::Metadata, e))?;
    if !opened.is_file() { return Err(TrajectoryPlanError::NotRegular); }
    if opened.len() > MAX_TRAJECTORY_PLAN_BYTES as u64 { return Err(TrajectoryPlanError::Limit); }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(opened.len() as usize).map_err(|_| TrajectoryPlanError::Limit)?;
    file.take(MAX_TRAJECTORY_PLAN_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(|e| fail(PlanFileStage::Read, e))?;
    if bytes.len() > MAX_TRAJECTORY_PLAN_BYTES { return Err(TrajectoryPlanError::Limit); }
    TrajectoryPlan::from_json(&bytes)
}
fn field(path: &str) -> TrajectoryPlanError { TrajectoryPlanError::Field(path.to_owned()) }
fn object<'a>(value: &'a Json, keys: &[&str], path: &str) -> Result<&'a BTreeMap<String, Json>, TrajectoryPlanError> {
    let value = value.as_object().ok_or_else(|| field(path))?;
    if value.len() != keys.len() || keys.iter().any(|key| !value.contains_key(*key)) { return Err(field(path)); }
    Ok(value)
}
fn integer(value: &Json, path: &str) -> Result<u64, TrajectoryPlanError> { value.as_u64().ok_or_else(|| field(path)) }
fn positive(value: &Json, path: &str) -> Result<u64, TrajectoryPlanError> {
    integer(value, path).and_then(|n| if n > 0 { Ok(n) } else { Err(field(path)) })
}
fn size(value: &Json, path: &str) -> Result<usize, TrajectoryPlanError> {
    usize::try_from(integer(value, path)?).map_err(|_| field(path))
}
