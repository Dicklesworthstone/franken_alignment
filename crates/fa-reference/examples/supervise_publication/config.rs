//! Explicit operator configuration for the runnable one-publication profile.
//! Parsing executes no program and opens no authority store. Original constructors
//! validate the policy, congress, helper contracts, source and two-key settings.
use fa_reference::action::{Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{PublicationEndpoint};
use fa_reference::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversightProfile;
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract, OversightBroker};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::action::consequence::oversight::human::HumanReviewPolicy;
use fa_reference::action::consequence::oversight::evidence_source::{FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const CONFIG_BYTES: usize = 2 * 1024 * 1024;
/// ASCII FAUNIXMS: elapsed ticks are explicitly Unix-epoch milliseconds.
/// This is a clock-domain label, not authentication or a monotonic-clock claim.
pub const CLOCK_DOMAIN: u64 = 0x4641_554e_4958_4d53;

#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub commit_ms: u64,
    pub reveal_ms: u64,
    pub runtime_ms: u64,
    pub poll_ms: u64,
    pub cleanup_ms: u64,
}

pub struct Config {
    pub store: PathBuf,
    pub profile: FileOversightProfile,
    pub source_policy: FileSourcePolicy,
    pub source: FileEvidenceSource,
    pub programs: BTreeMap<String, HelperProgram>,
    pub timing: Timing,
}
impl Config {
    pub fn read(path: &Path) -> Result<Self, String> { Self::decode(&read_regular(path, CONFIG_BYTES)?) }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let json = strict_json::parse(bytes, Limits { max_bytes: CONFIG_BYTES, max_depth: 12,
            max_items: 16_384, max_string_bytes: 1_048_576 }).map_err(|e| format!("configuration JSON: {e:?}"))?;
        let mut root = Fields::new(json, "configuration")?;
        if root.u64("version")? != 1 { return Err("unsupported configuration version".into()); }
        if root.text("clock")? != "unix_milliseconds" { return Err("clock must be unix_milliseconds".into()); }
        let store = absolute(root.text("store")?)?;
        let evidence_path = absolute(root.text("evidence_path")?)?;
        let scope = scope(root.take("scope")?)?;
        let target = target(root.take("target")?)?;
        let initial_payload = hex(&root.text("initial_payload_hex")?)?;
        let mut budget = root.object("budget")?;
        let total = budget.u64("total_units")?;
        let max_attempts = budget.usize("max_attempts")?;
        let max_deliveries = budget.usize("max_deliveries")?;
        let limits = JournalLimits { events: budget.usize("journal_events")?, bytes: budget.usize("journal_bytes")? };
        let suspend_at_incident = budget.u64("suspend_at_incident")?;
        let retention_ticks = budget.u64("retention_ms")?;
        budget.end()?;
        let reserve = RecoveryReserve::terminal();
        if limits.events <= reserve.events || limits.events > 4096 || limits.bytes <= reserve.bytes
            || limits.bytes > 16 * 1024 * 1024 { return Err("invalid journal limits".into()); }
        let actor = actor(root.object("actor")?)?;
        let policy = policy(root.object("policy")?)?;
        let mut congress = root.object("congress")?;
        let mut members = BTreeMap::new();
        let mut contracts = BTreeMap::new();
        let mut programs = BTreeMap::new();
        let helpers = object(root.take("helpers")?, "helpers")?;
        for (name, value) in helpers {
            let mut h = Fields::new(value, "helper")?;
            members.insert(name.clone(), MemberPolicy { cohort: h.text("cohort")?, weight: h.u64("weight")? });
            let mut p = h.object("profile")?;
            let input_profile = InputProfileBinding { profile_id: p.u64("id")?, profile_bytes: hex(&p.text("bytes_hex")?)?,
                tokenizer_epoch: p.u64("tokenizer_epoch")?, policy_epoch: 0, model_epoch: p.u64("model_epoch")? };
            p.end()?;
            contracts.insert(name.clone(), HelperContract::new(input_profile, h.u64("projection_id")?,
                hex(&h.text("question_hex")?)?).map_err(debug)?);
            let mut program = h.object("program")?;
            let executable = absolute(program.text("executable")?)?;
            let directory = absolute(program.text("directory")?)?;
            let arguments = array(program.take("arguments")?)?.into_iter()
                .map(|j| text(j).map(OsString::from)).collect::<Result<Vec<_>, _>>()?;
            let environment = object(program.take("environment")?, "environment")?.into_iter()
                .map(|(k, v)| Ok((OsString::from(k), OsString::from(text(v)?))))
                .collect::<Result<BTreeMap<_, _>, String>>()?;
            programs.insert(name, HelperProgram::new(executable, directory, arguments, environment).map_err(debug)?);
            program.end()?; h.end()?;
        }
        let congress_policy = CongressPolicy {
            generation: congress.u64("generation")?, members,
            caps: Caps { per_member: congress.u64("per_member_cap")?, per_cohort: congress.u64("per_cohort_cap")? },
            continue_minimum: congress.u64("continue_minimum")?, continue_hold_maximum: congress.u64("continue_hold_maximum")?,
            narrow_at: congress.u64("narrow_at")?, suspend_at: congress.u64("suspend_at")?,
            minimum_members: congress.usize("minimum_members")?, minimum_cohorts: congress.usize("minimum_cohorts")?,
        };
        congress.end()?;
        let mut human = root.object("human")?;
        let human_policy = HumanReviewPolicy { reviewer_id: human.u64("reviewer_id")?,
            max_validity_ticks: human.u64("max_validity_ms")?, max_requests: human.usize("max_requests")? };
        human.end()?;
        let mut source = root.object("source")?;
        let source_id = source.u64("id")?;
        let source_policy = FileSourcePolicy {
            source: StateSource { scope, source: source_id, generation: source.u64("capture_generation")? },
            limits: StateLimits { events: source.usize("events")?, retained_bytes: source.usize("retained_bytes")? },
            freshness: StateFreshness::new(source.u64("max_age_ms")?).map_err(debug)?,
        };
        let read_bytes = source.usize("read_bytes")?;
        if read_bytes > MAX_EVIDENCE_FILE_BYTES { return Err("evidence read limit too large".into()); }
        let source_reader = FileEvidenceSource::new(evidence_path, source_id, scope, read_bytes).map_err(debug)?;
        source.end()?;
        let mut timing = root.object("timing")?;
        let timing_value = Timing { commit_ms: timing.u64("commit_ms")?, reveal_ms: timing.u64("reveal_ms")?,
            runtime_ms: timing.u64("runtime_ms")?, poll_ms: timing.u64("poll_ms")?, cleanup_ms: timing.u64("cleanup_ms")? };
        if timing_value.commit_ms == 0 || timing_value.reveal_ms <= timing_value.commit_ms
            || timing_value.runtime_ms <= timing_value.reveal_ms || timing_value.runtime_ms > 86_400_000
            || !(1..=1000).contains(&timing_value.poll_ms) || !(1..=60_000).contains(&timing_value.cleanup_ms)
        { return Err("invalid finite timing profile".into()); }
        timing.end()?; root.end()?;
        let profile = FileOversightProfile {
            delivery: FileDeliveryProfile { scope, total, max_attempts, actor, suspend_at_incident, policy,
                congress: congress_policy, narrowed_targets: vec![target], target, initial_payload,
                retention_ticks, max_deliveries, clock_domain: CLOCK_DOMAIN, limits },
            committee: CommitteeContract::new(contracts).map_err(debug)?, human: human_policy,
        };
        // Validate the same full configuration in RAM before creating files. No
        // proposal, publication, helper inference or authority escapes this check.
        let d = &profile.delivery;
        let mut endpoint = PublicationEndpoint::new(d.target, d.initial_payload.clone(), d.retention_ticks, d.max_deliveries).map_err(debug)?;
        let mut broker = OversightBroker::new(ControllerConfig { scope: d.scope, total: d.total,
            max_attempts: d.max_attempts, actor: d.actor.clone(), suspend_at_incident: d.suspend_at_incident,
            policy: d.policy.clone(), congress: d.congress.clone(), narrowed_targets: TargetCeiling::new(&d.narrowed_targets).map_err(debug)? },
            &mut endpoint, profile.committee.clone()).map_err(debug)?;
        let _reviewer = broker.enable_human_review(profile.human).map_err(debug)?;
        let _writer = broker.enable_fresh_policy_state(source_policy.source, source_policy.limits, source_policy.freshness).map_err(debug)?;
        Ok(Self { store, profile, source_policy, source: source_reader, programs, timing: timing_value })
    }

    pub fn socket(&self, request: u64) -> PathBuf { self.store.join(format!("review-{request}.sock")) }
}

pub fn debug(error: impl std::fmt::Debug) -> String { format!("{error:?}") }
pub fn read_regular(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let before = fs::symlink_metadata(path).map_err(debug)?;
    if !before.is_file() || before.file_type().is_symlink() || before.len() > maximum as u64 {
        return Err(format!("not an admitted regular file: {path:?}"));
    }
    let file = File::open(path).map_err(debug)?;
    let opened = file.metadata().map_err(debug)?;
    if !opened.is_file() || opened.len() > maximum as u64 { return Err("opened file is outside limits".into()); }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1).read_to_end(&mut bytes).map_err(debug)?;
    if bytes.len() > maximum { return Err("file grew beyond its byte limit".into()); }
    Ok(bytes)
}
fn absolute(value: String) -> Result<PathBuf, String> {
    let p = PathBuf::from(value);
    if !p.is_absolute() { return Err("operator paths must be absolute".into()); }
    Ok(p)
}
fn scope(value: Json) -> Result<Scope, String> {
    let mut f = Fields::new(value, "scope")?;
    let value = Scope { tenant: f.u64("tenant")?, principal: f.u64("principal")?, run: f.u64("run")?,
        branch: f.u64("branch")?, authority: f.u64("authority")?, purpose: Purpose::Effect };
    f.end()?; Ok(value)
}
fn target(value: Json) -> Result<ResolvedTarget, String> {
    let mut f = Fields::new(value, "target")?;
    let value = ResolvedTarget { adapter: f.u64("adapter")?, object: f.u64("object")?,
        contract_version: f.u64("contract_version")?, expected_version: f.u64("expected_version")?, generation: f.u64("generation")? };
    f.end()?; Ok(value)
}
fn actor(mut f: Fields) -> Result<ActorState, String> {
    if f.text("grade")? != "audit_only" { return Err("command accepts audit_only actor metadata, not a model restart claim".into()); }
    let p = RestartProfile { id: f.u64("id")?, generation: f.u64("generation")?, host_generation: f.u64("host_generation")?,
        model_generation: f.u64("model_generation")?, tokenizer_generation: f.u64("tokenizer_generation")?,
        state_schema_generation: f.u64("state_schema_generation")?, grade: RestartGrade::AuditOnly };
    let tokens = array(f.take("tokens")?)?.iter().map(|j| {
        u32::try_from(j.as_u64().ok_or("token must be an unsigned integer")?).map_err(debug)
    }).collect::<Result<Vec<_>, String>>()?;
    let cache = hex(&f.text("cache_hex")?)?;
    let sampler = hex(&f.text("sampler_hex")?)?;
    let position = f.u64("next_position")?;
    f.end()?;
    ActorState::new(p, tokens, cache, sampler, position).map_err(debug)
}
fn policy(mut f: Fields) -> Result<Policy, String> {
    let generation = f.u64("generation")?;
    let raw = array(f.take("nodes")?)?; f.end()?;
    let mut nodes = Vec::new();
    for j in raw {
        let mut n = Fields::new(j, "policy node")?;
        let op = n.text("op")?;
        let node = match op.as_str() {
            "target_is" => Predicate::TargetIs(target(n.take("target")?)?),
            "payload_is" => Predicate::PayloadIs(hex(&n.text("value_hex")?)?),
            "payload_at_most" => Predicate::PayloadAtMost(n.usize("maximum")?),
            "units_at_most" => Predicate::UnitsAtMost(n.u64("maximum")?),
            "exact_value" => Predicate::ExactValue { key: n.u64("key")?, value: hex(&n.text("value_hex")?)? },
            "absent" => Predicate::Absent { key: n.u64("key")? },
            "empty_range" => Predicate::EmptyRange { start: n.u64("start")?, end: n.u64("end")? },
            "all" | "any" => {
                let all = op == "all";
                let children = array(n.take("children")?)?.iter().map(|j| {
                    usize::try_from(j.as_u64().ok_or("policy child must be unsigned")?).map_err(debug)
                }).collect::<Result<Vec<_>, String>>()?;
                if all { Predicate::All(children) } else { Predicate::Any(children) }
            }
            "not" => Predicate::Not(n.usize("child")?),
            _ => return Err("unsupported policy predicate".into()),
        };
        n.end()?; nodes.push(node);
    }
    Policy::new(generation, nodes).map_err(debug)
}
fn object(j: Json, at: &str) -> Result<BTreeMap<String, Json>, String> {
    match j { Json::Object(v) => Ok(v), _ => Err(format!("expected object at {at}")) }
}
fn array(j: Json) -> Result<Vec<Json>, String> { match j { Json::Array(v) => Ok(v), _ => Err("expected array".into()) } }
fn text(j: Json) -> Result<String, String> { match j { Json::String(v) => Ok(v), _ => Err("expected string".into()) } }
pub fn hex(text: &str) -> Result<Vec<u8>, String> {
    if text.len() % 2 != 0 { return Err("odd hex byte count".into()); }
    text.as_bytes().chunks_exact(2).map(|pair| {
        let digit = |c| match c { b'0'..=b'9' => Ok(c - b'0'), b'a'..=b'f' => Ok(c - b'a' + 10), _ => Err("hex must be lowercase".to_owned()) };
        Ok(digit(pair[0])? * 16 + digit(pair[1])?)
    }).collect()
}
struct Fields { values: BTreeMap<String, Json>, at: &'static str }
impl Fields {
    fn new(j: Json, at: &'static str) -> Result<Self, String> {
        let values = object(j, at)?;
        Ok(Self { values, at })
    }
    fn take(&mut self, name: &str) -> Result<Json, String> { self.values.remove(name).ok_or_else(|| format!("missing {}.{name}", self.at)) }
    fn text(&mut self, name: &str) -> Result<String, String> { text(self.take(name)?) }
    fn u64(&mut self, name: &str) -> Result<u64, String> { self.take(name)?.as_u64().ok_or_else(|| format!("{}.{} must be an unsigned integer", self.at, name)) }
    fn usize(&mut self, name: &str) -> Result<usize, String> { usize::try_from(self.u64(name)?).map_err(debug) }
    fn object(&mut self, name: &'static str) -> Result<Fields, String> { Fields::new(self.take(name)?, name) }
    fn end(self) -> Result<(), String> {
        if self.values.is_empty() { Ok(()) } else { Err(format!("unknown fields in {}: {:?}", self.at, self.values.keys().collect::<Vec<_>>())) }
    }
}
