//! Strict operator data for one original-token, all-layer training campaign.
//! No file paths, executable names, supplied scores or policy-promotion switches.

use super::{CampaignBudget, CampaignWork, CaptureBudget, CaptureWork, DecoderCampaign,
    DecoderCorpus, LabelledPrefix, LayerPolicy, MonitorExport, MAX_CAPTURE_TOKENS};
use super::super::{CaseLabel, CaseOrigin, DataSplit, FitPolicy, MAX_CORPUS_CASES};
use super::super::calibration::{CalibrationPolicy, ScreeningCriteria, MAX_THRESHOLD_CANDIDATES};
use crate::action::consequence::activation::{HEADER_BYTES, monitor::{RefinementBudget, MAX_LEVELS}};
use crate::action::consequence::activation::tensor::kv::{MAX_KV_POSITIONS, model::MAX_MODEL_KV_LAYERS};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderModel};
use crate::strict_json::{self, ErrorKind, Json, Limits};
use crate::Error;
use std::collections::BTreeMap;
use std::fmt;

pub const MAX_CAMPAIGN_PLAN_BYTES: usize = 4_194_304;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError { Syntax, Limit, Field(String), Contract(Error) }
impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for PlanError {}
impl From<Error> for PlanError { fn from(error: Error) -> Self { Self::Contract(error) } }

/// Frozen cases, thresholds and all fitting/evaluation/export settings. Only its
/// remaining work allowances change. Repeated runs do not renew those allowances.
pub struct ProbeRunPlan {
    id: u64,
    generation: u64,
    identity: DecoderIdentity,
    context: usize,
    cases: Vec<LabelledPrefix>,
    layers: BTreeMap<u64, LayerPolicy>,
    export: MonitorExport,
    capture: CaptureBudget,
    campaign: CampaignBudget,
}
impl fmt::Debug for ProbeRunPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProbeRunPlan").field("id", &self.id).field("generation", &self.generation)
            .field("cases", &self.cases.len()).field("layers", &self.layers.len()).finish_non_exhaustive()
    }
}
impl ProbeRunPlan {
    pub fn from_json(bytes: &[u8]) -> Result<Self, PlanError> {
        let value = strict_json::parse(bytes, Limits { max_bytes: MAX_CAMPAIGN_PLAN_BYTES,
            max_depth: 10, max_items: 600_000, max_string_bytes: 128 }).map_err(|error| match error.kind {
                ErrorKind::SizeLimit | ErrorKind::DepthLimit | ErrorKind::ItemLimit | ErrorKind::StringLimit => PlanError::Limit,
                _ => PlanError::Syntax,
            })?;
        let root = object(&value, &["schema", "id", "generation", "identity", "context", "capture_budget",
            "campaign_budget", "monitor", "cases", "layers"], "$")?;
        if root["schema"].as_str() != Some("fa.decoder-probe-campaign/1") { return Err(field("schema")); }
        let identity = object(&root["identity"], &["tenant", "model", "model_generation", "tokenizer_generation", "profile_generation"], "identity")?;
        let identity = DecoderIdentity { tenant: positive(&identity["tenant"], "identity.tenant")?,
            model: positive(&identity["model"], "identity.model")?, model_generation: positive(&identity["model_generation"], "identity.model_generation")?,
            tokenizer_generation: positive(&identity["tokenizer_generation"], "identity.tokenizer_generation")?,
            profile_generation: positive(&identity["profile_generation"], "identity.profile_generation")? };
        let context = size(&root["context"], "context")?;
        if context == 0 || context > MAX_KV_POSITIONS { return Err(field("context")); }
        let c = object(&root["capture_budget"], &["cases", "original_tokens", "residual_coordinates", "scalar_products"], "capture_budget")?;
        let capture = CaptureBudget::new(CaptureWork { cases: size(&c["cases"], "capture_budget.cases")?,
            original_tokens: size(&c["original_tokens"], "capture_budget.original_tokens")?,
            residual_coordinates: size(&c["residual_coordinates"], "capture_budget.residual_coordinates")?,
            scalar_products: integer(&c["scalar_products"], "capture_budget.scalar_products")? })?;
        let c = object(&root["campaign_budget"], &["training_visits", "scoring_bytes", "scoring_coordinates", "threshold_comparisons"], "campaign_budget")?;
        let campaign = CampaignBudget::new(CampaignWork { training_visits: integer(&c["training_visits"], "campaign_budget.training_visits")?,
            scoring_bytes: size(&c["scoring_bytes"], "campaign_budget.scoring_bytes")?,
            scoring_coordinates: size(&c["scoring_coordinates"], "campaign_budget.scoring_coordinates")?,
            threshold_comparisons: size(&c["threshold_comparisons"], "campaign_budget.threshold_comparisons")? })?;
        let m = object(&root["monitor"], &["generation", "levels", "per_layer_budget", "budget"], "monitor")?;
        let rungs = array(&m["levels"], "monitor.levels")?;
        if rungs.len() > MAX_LEVELS { return Err(PlanError::Limit); }
        let levels = rungs.iter().map(|v| {
            u8::try_from(integer(v, "monitor.levels")?).map_err(|_| field("monitor.levels"))
        }).collect::<Result<Vec<_>, _>>()?;
        let export = MonitorExport::new(positive(&m["generation"], "monitor.generation")?, levels,
            refinement(&m["per_layer_budget"], "monitor.per_layer_budget")?, refinement(&m["budget"], "monitor.budget")?)?;
        let entries = array(&root["cases"], "cases")?;
        if entries.is_empty() || entries.len() > MAX_CORPUS_CASES { return Err(field("cases")); }
        let mut cases = Vec::new();
        let mut token_count = 0_usize;
        for value in entries {
            let c = object(value, &["task", "lineage", "split", "label", "tokens"], "cases[]")?;
            let values = array(&c["tokens"], "cases[].tokens")?;
            token_count = token_count.checked_add(values.len()).ok_or(PlanError::Limit)?;
            if values.is_empty() || values.len() > context || token_count > MAX_CAPTURE_TOKENS { return Err(field("cases[].tokens")); }
            let tokens = values.iter().map(|v| u32::try_from(integer(v, "cases[].tokens")?).map_err(|_| field("cases[].tokens")))
                .collect::<Result<Vec<_>, _>>()?;
            cases.push(LabelledPrefix { origin: CaseOrigin { task: positive(&c["task"], "cases[].task")?,
                lineage: positive(&c["lineage"], "cases[].lineage")? },
                split: match c["split"].as_str() {
                    Some("training") => DataSplit::Training, Some("calibration") => DataSplit::Calibration,
                    Some("evaluation") => DataSplit::Evaluation, _ => return Err(field("cases[].split")),
                },
                label: match c["label"].as_str() {
                    Some("benign") => CaseLabel::Benign, Some("violation") => CaseLabel::Violation,
                    _ => return Err(field("cases[].label")),
                }, tokens });
        }
        let entries = array(&root["layers"], "layers")?;
        if entries.is_empty() || entries.len() > MAX_MODEL_KV_LAYERS { return Err(field("layers")); }
        let mut layers = BTreeMap::new();
        for value in entries {
            let layer = object(value, &["layer", "fit", "calibration"], "layers[]")?;
            let f = object(&layer["fit"], &["id", "generation", "epochs", "learning_rate", "l2", "scale_floor"], "layers[].fit")?;
            let fit = FitPolicy::new(positive(&f["id"], "fit.id")?, positive(&f["generation"], "fit.generation")?,
                size(&f["epochs"], "fit.epochs")?, number(&f["learning_rate"], "fit.learning_rate")?,
                number(&f["l2"], "fit.l2")?, number(&f["scale_floor"], "fit.scale_floor")?)?;
            let c = object(&layer["calibration"], &["id", "generation", "thresholds", "calibration", "evaluation"], "layers[].calibration")?;
            let candidates = array(&c["thresholds"], "calibration.thresholds")?;
            if candidates.len() > MAX_THRESHOLD_CANDIDATES { return Err(PlanError::Limit); }
            let thresholds = candidates.iter().map(|v| match v {
                Json::Number(n) => n.lexeme().parse::<f32>().ok().filter(|v| v.is_finite()).ok_or_else(|| field("calibration.thresholds")),
                _ => Err(field("calibration.thresholds")),
            }).collect::<Result<Vec<_>, _>>()?;
            let calibration = CalibrationPolicy::new(positive(&c["id"], "calibration.id")?,
                positive(&c["generation"], "calibration.generation")?, &thresholds,
                criteria(&c["calibration"])?, criteria(&c["evaluation"])?)?;
            if layers.insert(positive(&layer["layer"], "layers[].layer")?, LayerPolicy { fit, calibration }).is_some() {
                return Err(Error::Duplicate.into());
            }
        }
        Ok(Self { id: positive(&root["id"], "id")?, generation: positive(&root["generation"], "generation")?,
            identity, context, cases, layers, export, capture, campaign })
    }

    pub fn identity(&self) -> DecoderIdentity { self.identity }
    pub fn context(&self) -> usize { self.context }
    pub fn settings(&self) -> &MonitorExport { &self.export }
    pub fn cases(&self) -> &[LabelledPrefix] { &self.cases }
    pub fn remaining_capture(&self) -> CaptureWork { self.capture.remaining() }
    pub fn remaining_campaign(&self) -> CampaignWork { self.campaign.remaining() }

    /// Preflight BOTH stages before any inference. All fitted layer dimensions
    /// are fixed by the actual model, never by a supplied activation array.
    pub fn estimate(&self, model: &DecoderModel) -> Result<(CaptureWork, CampaignWork), Error> {
        if model.profile().identity() != self.identity || model.profile().shape().context != self.context {
            return Err(Error::Binding);
        }
        let capture = DecoderCorpus::estimate(model, &self.cases)?;
        if !self.layers.keys().copied().eq(1..=model.profile().shape().layers as u64) { return Err(Error::Binding); }
        let counts = |split| self.cases.iter().filter(|case| case.split == split).count();
        let training = counts(DataSplit::Training) as u64;
        let calibration = counts(DataSplit::Calibration);
        let evaluation = counts(DataSplit::Evaluation);
        let dimensions = model.profile().shape().hidden;
        let scoring_cases = calibration.checked_add(evaluation).ok_or(Error::Overflow)?;
        let per_frame = dimensions.checked_mul(4).and_then(|n| n.checked_add(HEADER_BYTES)).ok_or(Error::Overflow)?;
        let mut work = CampaignWork::default();
        for policy in self.layers.values() {
            let visits = training.checked_mul(dimensions as u64)
                .and_then(|n| n.checked_mul(1 + 2 * policy.fit.epochs() as u64)).ok_or(Error::Overflow)?;
            work.training_visits = work.training_visits.checked_add(visits).ok_or(Error::Overflow)?;
            work.scoring_bytes = work.scoring_bytes.checked_add(scoring_cases.checked_mul(per_frame).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
            work.scoring_coordinates = work.scoring_coordinates.checked_add(scoring_cases.checked_mul(dimensions).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
            work.threshold_comparisons = work.threshold_comparisons.checked_add(calibration.checked_mul(policy.calibration.thresholds().count())
                .and_then(|n| n.checked_add(evaluation)).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
        }
        CampaignBudget::new(work)?;
        let c = self.capture.remaining(); let w = self.campaign.remaining();
        if capture.cases > c.cases || capture.original_tokens > c.original_tokens
            || capture.residual_coordinates > c.residual_coordinates || capture.scalar_products > c.scalar_products
            || work.training_visits > w.training_visits || work.scoring_bytes > w.scoring_bytes
            || work.scoring_coordinates > w.scoring_coordinates || work.threshold_comparisons > w.threshold_comparisons {
            return Err(Error::Limit);
        }
        Ok((capture, work))
    }

    pub fn run(&mut self, model: &DecoderModel) -> Result<DecoderCampaign, Error> {
        let (_, expected) = self.estimate(model)?;
        let corpus = DecoderCorpus::capture(model, self.id, self.generation, &self.cases, &mut self.capture)?;
        if corpus.estimate_campaign(&self.layers)? != expected { return Err(Error::Binding); }
        corpus.run(self.layers.clone(), &mut self.campaign)
    }
}

fn field(path: &str) -> PlanError { PlanError::Field(path.to_owned()) }
fn object<'a>(value: &'a Json, keys: &[&str], path: &str) -> Result<&'a BTreeMap<String, Json>, PlanError> {
    let o = value.as_object().ok_or_else(|| field(path))?;
    if o.len() != keys.len() || keys.iter().any(|key| !o.contains_key(*key)) { return Err(field(path)); }
    Ok(o)
}
fn array<'a>(value: &'a Json, path: &str) -> Result<&'a [Json], PlanError> { value.as_array().ok_or_else(|| field(path)) }
fn integer(value: &Json, path: &str) -> Result<u64, PlanError> { value.as_u64().ok_or_else(|| field(path)) }
fn positive(value: &Json, path: &str) -> Result<u64, PlanError> {
    integer(value, path).and_then(|n| if n == 0 { Err(field(path)) } else { Ok(n) })
}
fn size(value: &Json, path: &str) -> Result<usize, PlanError> { usize::try_from(integer(value, path)?).map_err(|_| field(path)) }
fn number(value: &Json, path: &str) -> Result<f64, PlanError> {
    match value {
        Json::Number(n) => n.lexeme().parse::<f64>().ok().filter(|n| n.is_finite()).ok_or_else(|| field(path)),
        _ => Err(field(path)),
    }
}
fn refinement(value: &Json, path: &str) -> Result<RefinementBudget, PlanError> {
    let o = object(value, &["encoded_bytes", "probe_coordinates"], path)?;
    Ok(RefinementBudget { encoded_bytes: size(&o["encoded_bytes"], path)?, probe_coordinates: size(&o["probe_coordinates"], path)? })
}
fn criteria(value: &Json) -> Result<ScreeningCriteria, PlanError> {
    let o = object(value, &["min_violation_alarms", "max_benign_holds"], "criteria")?;
    Ok(ScreeningCriteria::new(size(&o["min_violation_alarms"], "criteria.min_violation_alarms")?,
        size(&o["max_benign_holds"], "criteria.max_benign_holds")?)?)
}
