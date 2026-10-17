//! Closed operator campaign data. Every case, tap, threshold and cap is explicit.
use super::{fields::Fields, config::debug};
use fa_reference::action::consequence::activation::{HEADER_BYTES,
    monitor::learned::{LearnedMonitorBudget, MAX_LEARNED_MONITOR_COORDINATES,
        model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, MAX_LEARNED_AUDIT_ROWS}},
    probe::{learned::{CheckedKvBudget, MAX_CHECKED_KV_BYTES, MAX_CHECKED_KV_PRODUCTS, MAX_CHECKED_KV_GROUPS},
        training::{CaseLabel, CaseOrigin, ClassCounts, DataSplit, FitPolicy, MAX_CORPUS_CASES,
            calibration::{CalibrationPolicy, ScreeningCriteria, MAX_THRESHOLD_CANDIDATES},
            decoder::{LabelledPrefix, LayerPolicy, MAX_CAPTURE_TOKENS,
                kv::{KvCaptureBudget, KvCaptureWork, KvCampaignBudget, KvCampaignWork,
                    KvDecoderCorpus, KvDecoderCampaign, KvMonitorSettings, MAX_KV_MONITOR_ARTIFACT_BYTES}}}},
    tensor::kv::{MAX_KV_POSITIONS, experiment::KvSide,
        decoder::{DecoderIdentity, DecoderModel, DecoderProfile, DecoderBudget, MAX_DECODER_PRODUCTS,
            monitoring::LearnedStreamRetention,
            safetensors::{MAX_WEIGHT_FILE_BYTES, reader::MAX_WEIGHT_READ_CALLS}},
        model::{MAX_MODEL_KV_LAYERS, MAX_MODEL_KV_VALUES,
            learned::{CompressionBudget, MAX_LEARNED_IMAGE_BYTES, MAX_COMPRESSION_WORK,
                replay::archive::{MAX_FIT_ARCHIVE_BYTES, reader::MAX_FIT_ARCHIVE_READ_CALLS}}}}};
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::NativeTokenizerFormat;
use fa_reference::strict_json::Json;
use fa_reference::Error;
use std::collections::{BTreeMap, BTreeSet};

pub(super) const MAX_PLAN_BYTES: usize = 4 * 1024 * 1024;
pub(super) const MAX_REPORT_BYTES: usize = 64 * 1024 * 1024;

pub(super) struct InputLimits {
    pub auxiliary_bytes: usize,
    pub weight_bytes: usize,
    pub weight_read_calls: usize,
    pub codec_bytes: usize,
    pub codec_read_calls: usize,
}

pub(super) struct Plan {
    pub id: u64,
    pub generation: u64,
    pub identity: DecoderIdentity,
    pub context: usize,
    pub tokenizer_format: NativeTokenizerFormat,
    pub input_limits: InputLimits,
    pub report_bytes: usize,
    pub monitor_bytes: usize,
    pub cases: Vec<LabelledPrefix>,
    pub taps: BTreeMap<KvTap, LayerPolicy>,
    pub settings: KvMonitorSettings,
    capture: KvCaptureBudget,
    campaign: KvCampaignBudget,
    captured: Option<KvCaptureWork>,
    started: bool,
}

impl Plan {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut root = Fields::parse(bytes, MAX_PLAN_BYTES)?;
        if root.text("schema")? != "fa.learned-kv-probe-campaign/1" { return Err("unsupported K/V probe campaign schema".into()); }
        let id = positive(&mut root, "id")?;
        let generation = positive(&mut root, "generation")?;
        let mut identity = root.object("identity")?;
        let selected_identity = DecoderIdentity {
            tenant: positive(&mut identity, "tenant")?, model: positive(&mut identity, "model")?,
            model_generation: positive(&mut identity, "model_generation")?,
            tokenizer_generation: positive(&mut identity, "tokenizer_generation")?,
            profile_generation: positive(&mut identity, "profile_generation")?,
        };
        identity.end()?;
        let context = root.size("context")?;
        if context == 0 || context > MAX_KV_POSITIONS { return Err("invalid campaign context".into()); }
        let tokenizer_format = match root.text("tokenizer_format")?.as_str() {
            "native_archive" => NativeTokenizerFormat::NativeArchive,
            "huggingface_raw_bytelevel" => NativeTokenizerFormat::HuggingFaceRawByteLevel,
            "huggingface_gpt2_unicode17" => NativeTokenizerFormat::HuggingFaceGpt2Unicode17,
            "huggingface_llama3_unicode17" => NativeTokenizerFormat::HuggingFaceLlama3Unicode17,
            "huggingface_llama3_unicode17_bos" => NativeTokenizerFormat::HuggingFaceLlama3Unicode17Bos,
            _ => return Err("unsupported explicit tokenizer format".into()),
        };
        let mut limits = root.object("input_limits")?;
        let input_limits = InputLimits { auxiliary_bytes: limits.size("auxiliary_bytes")?,
            weight_bytes: limits.size("weight_bytes")?, weight_read_calls: limits.size("weight_read_calls")?,
            codec_bytes: limits.size("codec_bytes")?, codec_read_calls: limits.size("codec_read_calls")? };
        limits.end()?;
        if input_limits.auxiliary_bytes == 0 || input_limits.auxiliary_bytes > super::fields::MAX_ASSET_BYTES
            || input_limits.weight_bytes == 0 || input_limits.weight_bytes > MAX_WEIGHT_FILE_BYTES
            || input_limits.weight_read_calls == 0 || input_limits.weight_read_calls > MAX_WEIGHT_READ_CALLS
            || input_limits.codec_bytes == 0 || input_limits.codec_bytes > MAX_FIT_ARCHIVE_BYTES
            || input_limits.codec_read_calls == 0 || input_limits.codec_read_calls > MAX_FIT_ARCHIVE_READ_CALLS {
            return Err("input limits exceed the original bounded reader profiles".into());
        }
        let mut limits = root.object("output_limits")?;
        let report_bytes = limits.size("report_bytes")?;
        let monitor_bytes = limits.size("monitor_bytes")?;
        limits.end()?;
        if report_bytes == 0 || report_bytes > MAX_REPORT_BYTES || monitor_bytes == 0
            || monitor_bytes > MAX_KV_MONITOR_ARTIFACT_BYTES { return Err("invalid campaign output limits".into()); }
        let mut c = root.object("capture_budget")?;
        let capture = KvCaptureBudget::new(KvCaptureWork { cases: c.size("cases")?,
            original_tokens: c.size("original_tokens")?, kv_coordinates: c.size("kv_coordinates")?,
            scalar_products: c.number("scalar_products")? }).map_err(debug)?;
        c.end()?;
        let mut c = root.object("campaign_budget")?;
        let campaign = KvCampaignBudget::new(KvCampaignWork { training_visits: c.number("training_visits")?,
            scoring_bytes: c.size("scoring_bytes")?, scoring_coordinates: c.size("scoring_coordinates")?,
            threshold_comparisons: c.size("threshold_comparisons")? }).map_err(debug)?;
        c.end()?;
        let mut monitor = root.object("monitor")?;
        let retention = match monitor.text("retention")?.as_str() {
            "all" => LearnedStreamRetention::All, "none" => LearnedStreamRetention::None,
            _ => return Err("unsupported explicit residual retention".into()),
        };
        let inference = DecoderBudget { scalar_products: monitor.number("inference_products")? };
        if inference.scalar_products > MAX_DECODER_PRODUCTS { return Err("invalid inference product cap".into()); }
        let mut prep = monitor.object("preparation")?;
        let mut compression = prep.object("compression")?;
        let compression_value = CompressionBudget { source_values: compression.size("source_values")?,
            encoded_bytes: compression.size("encoded_bytes")?, work_units: compression.number("work_units")? };
        compression.end()?;
        let mut source = prep.object("source_check")?;
        let checked = CheckedKvBudget { source_values: source.size("source_values")?,
            encoded_bytes: source.size("encoded_bytes")?, reconstruction_products: source.number("reconstruction_products")? };
        source.end()?; prep.end()?;
        if compression_value.source_values > MAX_MODEL_KV_VALUES || compression_value.encoded_bytes > MAX_LEARNED_IMAGE_BYTES
            || compression_value.work_units > MAX_COMPRESSION_WORK || checked.source_values > MAX_MODEL_KV_VALUES
            || checked.encoded_bytes > MAX_CHECKED_KV_BYTES || checked.reconstruction_products > MAX_CHECKED_KV_PRODUCTS {
            return Err("invalid original learned preparation cap".into());
        }
        let mut audit = monitor.object("audit")?;
        let audit_value = LearnedAuditBudget { rows: audit.size("rows")?, monitoring: monitoring(audit.object("budget")?)? };
        audit.end()?; monitor.end()?;
        if audit_value.rows > MAX_LEARNED_AUDIT_ROWS { return Err("invalid original audit row cap".into()); }
        let mut cases = Vec::new();
        let mut token_count = 0_usize;
        let mut tasks = BTreeSet::new(); let mut lineages = BTreeSet::new(); let mut histories = BTreeSet::new();
        let mut counts: BTreeMap<DataSplit, ClassCounts> = BTreeMap::new();
        for value in root.array("cases", MAX_CORPUS_CASES)? {
            let mut case = Fields::new(value)?;
            let origin = CaseOrigin { task: positive(&mut case, "task")?, lineage: positive(&mut case, "lineage")? };
            let split = match case.text("split")?.as_str() {
                "training" => DataSplit::Training, "calibration" => DataSplit::Calibration,
                "evaluation" => DataSplit::Evaluation, _ => return Err("unknown case split".into()),
            };
            let label = match case.text("label")?.as_str() {
                "benign" => CaseLabel::Benign, "violation" => CaseLabel::Violation, _ => return Err("unknown case label".into()),
            };
            let tokens = case.ids("tokens", context)?; case.end()?;
            token_count = token_count.checked_add(tokens.len()).ok_or("too many original tokens")?;
            if tokens.is_empty() || token_count > MAX_CAPTURE_TOKENS { return Err("invalid complete token population".into()); }
            if !tasks.insert(origin.task) || !lineages.insert(origin.lineage) || !histories.insert(tokens.clone()) {
                return Err("duplicate task, lineage or exact history in frozen population".into());
            }
            let count = counts.entry(split).or_default();
            match label { CaseLabel::Benign => count.benign += 1, CaseLabel::Violation => count.violation += 1 }
            cases.push(LabelledPrefix { origin, split, label, tokens });
        }
        if counts.len() != 3 || counts.values().any(|c| c.benign == 0 || c.violation == 0) {
            return Err("both labels are required in every one of the three splits".into());
        }
        cases.sort_by_key(|case| case.origin);
        let mut taps = BTreeMap::new(); let mut budgets = BTreeMap::new();
        for value in root.array("taps", 2 * MAX_MODEL_KV_LAYERS)? {
            let mut tap = Fields::new(value)?;
            let layer = positive(&mut tap, "layer")?;
            let side = match tap.text("side")?.as_str() {
                "key" => KvSide::Key, "value" => KvSide::Value, _ => return Err("unknown K/V tap side".into()),
            };
            let mut f = tap.object("fit")?;
            let fitted = FitPolicy::new(positive(&mut f, "id")?, positive(&mut f, "generation")?,
                f.size("epochs")?, real(f.take("learning_rate")?)?, real(f.take("l2")?)?, real(f.take("scale_floor")?)?).map_err(debug)?;
            f.end()?;
            let mut c = tap.object("calibration")?;
            let calibrated = CalibrationPolicy::new(positive(&mut c, "id")?, positive(&mut c, "generation")?,
                &c.scalars("thresholds", MAX_THRESHOLD_CANDIDATES)?, criteria(c.object("calibration")?)?,
                criteria(c.object("evaluation")?)?).map_err(debug)?;
            c.end()?;
            let budget = monitoring(tap.object("monitor_budget")?)?;
            tap.end()?;
            let key = KvTap { layer, side };
            if taps.insert(key, LayerPolicy { fit: fitted, calibration: calibrated }).is_some() {
                return Err("duplicate mandatory K/V tap".into());
            }
            budgets.insert(key, budget);
        }
        root.end()?;
        if taps.is_empty() { return Err("complete K/V tap inventory is required".into()); }
        Ok(Self { id, generation, identity: selected_identity, context, tokenizer_format, input_limits,
            report_bytes, monitor_bytes, cases, taps, capture, campaign, captured: None, started: false,
            settings: KvMonitorSettings { taps: budgets, audit: audit_value,
                preparation: LearnedAuditPreparationBudget { compression: compression_value, source_check: checked }, inference, retention } })
    }

    pub fn remaining_capture(&self) -> KvCaptureWork { self.capture.remaining() }
    pub fn remaining_campaign(&self) -> KvCampaignWork { self.campaign.remaining() }
    pub fn captured_work(&self) -> Option<KvCaptureWork> { self.captured }
    pub fn started(&self) -> bool { self.started }

    /// Whole-model preflight uses the original decoder's own profile estimator.
    /// Fit/scoring loop counts are checked against the original captured corpus
    /// before the original campaign is allowed to execute.
    pub fn estimate(&self, profile: &DecoderProfile) -> Result<(KvCaptureWork, KvCampaignWork), Error> {
        if self.started { return Err(Error::WrongState); }
        if profile.identity() != self.identity || profile.shape().context != self.context { return Err(Error::Binding); }
        let shape = profile.shape();
        let required = (1..=shape.layers as u64).flat_map(|layer|
            [KvSide::Key, KvSide::Value].map(move |side| KvTap { layer, side }));
        if !self.taps.keys().copied().eq(required) { return Err(Error::Binding); }
        let width = profile.cache_width();
        let mut capture = KvCaptureWork { cases: self.cases.len(),
            kv_coordinates: self.cases.len().checked_mul(shape.layers).and_then(|n| n.checked_mul(2))
                .and_then(|n| n.checked_mul(width)).ok_or(Error::Overflow)?, ..KvCaptureWork::default() };
        for case in &self.cases {
            if case.tokens.iter().any(|token| *token as usize >= shape.vocabulary) { return Err(Error::InvalidInput); }
            capture.original_tokens = capture.original_tokens.checked_add(case.tokens.len()).ok_or(Error::Overflow)?;
            capture.scalar_products = capture.scalar_products.checked_add(profile.estimate(0, case.tokens.len())?.scalar_products()?)
                .ok_or(Error::Overflow)?;
        }
        KvCaptureBudget::new(capture)?;
        let count = |split| self.cases.iter().filter(|case| case.split == split).count();
        let train = count(DataSplit::Training) as u64;
        let calibration = count(DataSplit::Calibration);
        let evaluation = count(DataSplit::Evaluation);
        let scored = calibration.checked_add(evaluation).ok_or(Error::Overflow)?;
        let frame = width.checked_mul(4).and_then(|n| n.checked_add(HEADER_BYTES)).ok_or(Error::Overflow)?;
        let mut campaign = KvCampaignWork::default();
        for policy in self.taps.values() {
            let visits = train.checked_mul(width as u64).and_then(|n| n.checked_mul(1 + 2 * policy.fit.epochs() as u64)).ok_or(Error::Overflow)?;
            campaign.training_visits = campaign.training_visits.checked_add(visits).ok_or(Error::Overflow)?;
            campaign.scoring_bytes = campaign.scoring_bytes.checked_add(scored.checked_mul(frame).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
            campaign.scoring_coordinates = campaign.scoring_coordinates.checked_add(scored.checked_mul(width).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
            campaign.threshold_comparisons = campaign.threshold_comparisons.checked_add(calibration.checked_mul(policy.calibration.thresholds().count())
                .and_then(|n| n.checked_add(evaluation)).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;
        }
        KvCampaignBudget::new(campaign)?;
        let c = self.capture.remaining(); let w = self.campaign.remaining();
        if capture.cases > c.cases || capture.original_tokens > c.original_tokens || capture.kv_coordinates > c.kv_coordinates
            || capture.scalar_products > c.scalar_products || campaign.training_visits > w.training_visits
            || campaign.scoring_bytes > w.scoring_bytes || campaign.scoring_coordinates > w.scoring_coordinates
            || campaign.threshold_comparisons > w.threshold_comparisons { return Err(Error::Limit); }
        if self.report_bound(width)? > self.report_bytes { return Err(Error::Limit); }
        Ok((capture, campaign))
    }

    fn report_bound(&self, width: usize) -> Result<usize, Error> {
        // Conservative byte reservation for every fixed case, fitted coefficient,
        // exact score word, threshold trial and terminal diagnostic. This bounds
        // reporting storage, not memory or detector quality.
        let mut bytes = 16_384_usize;
        for case in &self.cases {
            bytes = bytes.checked_add(512).and_then(|n| n.checked_add(case.tokens.len().checked_mul(12)?)).ok_or(Error::Overflow)?;
        }
        let scored = self.cases.iter().filter(|case| case.split != DataSplit::Training).count();
        for policy in self.taps.values() {
            bytes = bytes.checked_add(4096).and_then(|n| n.checked_add(width.checked_mul(128)?))
                .and_then(|n| n.checked_add(scored.checked_mul(2048)?))
                .and_then(|n| n.checked_add(policy.calibration.thresholds().count().checked_mul(1024)?)).ok_or(Error::Overflow)?;
        }
        Ok(bytes)
    }

    pub fn run(&mut self, model: &DecoderModel) -> Result<KvDecoderCampaign, Error> {
        let (capture_work, campaign_work) = self.estimate(model.profile())?;
        if KvDecoderCorpus::estimate(model, &self.cases)? != capture_work { return Err(Error::Binding); }
        self.started = true;
        let corpus = KvDecoderCorpus::capture(model, self.id, self.generation, &self.cases, &mut self.capture)?;
        self.captured = Some(corpus.work());
        if corpus.estimate_campaign(&self.taps)? != campaign_work { return Err(Error::Binding); }
        corpus.run(self.taps.clone(), &mut self.campaign)
    }
}

fn positive(fields: &mut Fields, name: &str) -> Result<u64, String> {
    let n = fields.number(name)?;
    if n == 0 { Err(format!("{name} must be nonzero")) } else { Ok(n) }
}
fn real(value: Json) -> Result<f64, String> {
    let Json::Number(n) = value else { return Err("expected finite number".into()); };
    n.lexeme().parse::<f64>().ok().filter(|n| n.is_finite()).ok_or_else(|| "expected finite number".into())
}
fn criteria(mut value: Fields) -> Result<ScreeningCriteria, String> {
    let result = ScreeningCriteria::new(value.size("min_violation_alarms")?, value.size("max_benign_holds")?).map_err(debug)?;
    value.end()?; Ok(result)
}
fn monitoring(mut value: Fields) -> Result<LearnedMonitorBudget, String> {
    let budget = LearnedMonitorBudget { encoded_bytes: value.size("encoded_bytes")?, probe_coordinates: value.size("probe_coordinates")?,
        reconstruction_products: value.number("reconstruction_products")?, materialized_values: value.size("materialized_values")?,
        refinements: value.size("refinements")? };
    value.end()?;
    if budget.encoded_bytes > MAX_CHECKED_KV_BYTES || budget.probe_coordinates > MAX_LEARNED_MONITOR_COORDINATES
        || budget.reconstruction_products > MAX_CHECKED_KV_PRODUCTS || budget.materialized_values > MAX_MODEL_KV_VALUES
        || budget.refinements > MAX_CHECKED_KV_GROUPS { return Err("invalid original learned monitor cap".into()); }
    Ok(budget)
}
