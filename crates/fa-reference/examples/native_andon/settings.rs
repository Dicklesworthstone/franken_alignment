//! Closed, bounded configuration. No stored scores or caller-made receipts.
use crate::config::{Config, debug, read_regular};
use fa_reference::action::consequence::delivery::persistent::observed::{
    credibility::andon::FileAndonConfig,
    guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
        evaluation::andon::FileAndonRecoveryRequirements},
};
use fa_reference::action::consequence::delivery::persistent::{RecoveryReserve,
    observed::{FileOversight, guarded::evaluation::FileEvaluatedOversightRoles}};
use fa_reference::action::consequence::oversight::joint_credibility::{JointPromotionPolicy, JointReplayBudget};
use fa_reference::action::consequence::oversight::credibility::{EvaluationProtocol, Fraction};
use fa_reference::round::review::andon::{AndonBudget, LedgerIdentity, MAX_WINDOWS};
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeMap, path::Path};

const MAX_BYTES: usize = 64 * 1024;

pub(super) struct Settings {
    pub(super) protocol: EvaluationProtocol,
    pub(super) config: FileAndonConfig,
    pub(super) windows: Vec<AndonBudget>,
    pub(super) minimum_evaluation_revision: u64,
    pub(super) joint: Option<JointPromotionPolicy>,
}
impl Settings {
    pub(super) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, MAX_BYTES)?)
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let json = strict_json::parse(bytes, Limits { max_bytes: MAX_BYTES, max_depth: 6,
            max_items: 4096, max_string_bytes: 256 }).map_err(debug)?;
        let mut root = object(json)?;
        let joint = match take(&mut root, "schema_version")?.as_str() {
            Some("fa.native-andon-operator/1") => None,
            Some("fa.native-andon-operator/2") => Some(joint_policy(take(&mut root, "joint")?)?),
            _ => return Err("unsupported native Andon operator profile".into()),
        };
        let mut p = object(take(&mut root, "evaluation")?)?;
        let protocol = EvaluationProtocol {
            domain: number(&mut p, "domain")?, stratum: number(&mut p, "stratum")?,
            period: number(&mut p, "period")?,
            minimum_violation_origins: number(&mut p, "minimum_violation_origins")?,
            minimum_benign_origins: number(&mut p, "minimum_benign_origins")?,
            precision_floor: fraction(take(&mut p, "precision_floor")?)?,
            recall_floor: fraction(take(&mut p, "recall_floor")?)?,
            false_positive_ceiling: fraction(take(&mut p, "false_positive_ceiling")?)?,
            false_stop_budget: number(&mut p, "false_stop_budget")?,
        };
        end(p)?;
        let mut a = object(take(&mut root, "accounting")?)?;
        let identity = LedgerIdentity { deployment: number(&mut a, "deployment")?,
            evaluator: number(&mut a, "evaluator")? };
        let config = FileAndonConfig::new(identity, windows(take(&mut a, "initial_windows")?)?)
            .map_err(debug)?;
        end(a)?;
        let current = FileAndonConfig::new(identity, windows(take(&mut root, "current_windows")?)?)
            .map_err(debug)?;
        let minimum_evaluation_revision = number(&mut root, "minimum_evaluation_revision")?;
        end(root)?;
        // The final inventory must contain every initial immutable window.
        for initial in config.windows() {
            if !current.windows().contains(initial) {
                return Err("current windows omit or change an initial window".into());
            }
        }
        Ok(Self { protocol, config, windows: current.windows().to_vec(), minimum_evaluation_revision, joint })
    }
    pub(super) fn creation(&self) -> Result<(), String> {
        if self.minimum_evaluation_revision != 0 || self.windows != self.config.windows() {
            return Err("creation requires the initial inventory and evaluation revision zero".into());
        }
        Ok(())
    }
    pub(super) fn requirements(&self, config: &Config, minimum: FileRecoveryFloor)
        -> Result<FileAndonRecoveryRequirements, String>
    {
        let expected = FileAndonRecoveryRequirements::new(FileRecoveryRequirements {
            guards: guards(config), effective_policy: config.profile.delivery.policy.clone(),
            credential_epoch: None, minimum,
        }, self.config.clone(), self.windows.clone(), self.minimum_evaluation_revision).map_err(debug)?;
        match self.joint { Some(policy) => expected.require_joint(policy).map_err(debug), None => Ok(expected) }
    }

    /// Both executable consumers select the exact same native first-image profile.
    /// A /2 error never falls back to /1, including reserved live publication.
    pub(super) fn create(&self, config: &Config, reserve: Option<RecoveryReserve>)
        -> Result<(FileOversight, FileEvaluatedOversightRoles), String>
    {
        self.creation()?;
        match (self.joint, reserve) {
            (Some(policy), Some(reserve)) => FileOversight::create_joint_andon_guarded_reserved(
                &config.store, config.profile.clone(), &guards(config), None,
                self.protocol.clone(), policy, &self.config, reserve),
            (Some(policy), None) => FileOversight::create_joint_andon_guarded(
                &config.store, config.profile.clone(), &guards(config), None,
                self.protocol.clone(), policy, &self.config),
            (None, Some(reserve)) => FileOversight::create_andon_guarded_reserved(
                &config.store, config.profile.clone(), &guards(config), None,
                self.protocol.clone(), &self.config, reserve),
            (None, None) => FileOversight::create_andon_guarded(
                &config.store, config.profile.clone(), &guards(config), None,
                self.protocol.clone(), &self.config),
        }.map_err(debug)
    }
}

// Exact ordinary source-publication inventory. Optional numerical,
// identity, campaign and credential profiles are not silently accepted.
pub(super) fn guards(config: &Config) -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None,
        source: Some(config.source_policy), identity: None, campaigns: None, credential: None }
}
pub(super) fn object(value: Json) -> Result<BTreeMap<String, Json>, String> {
    match value { Json::Object(fields) => Ok(fields), _ => Err("expected Andon profile object".into()) }
}
pub(super) fn take(fields: &mut BTreeMap<String, Json>, key: &str) -> Result<Json, String> {
    fields.remove(key).ok_or_else(|| format!("missing Andon profile field {key}"))
}
pub(super) fn number(fields: &mut BTreeMap<String, Json>, key: &str) -> Result<u64, String> {
    take(fields, key)?.as_u64().ok_or_else(|| format!("{key} must be an unsigned integer"))
}
pub(super) fn end(fields: BTreeMap<String, Json>) -> Result<(), String> {
    if fields.is_empty() { Ok(()) } else { Err("unknown Andon profile fields".into()) }
}
fn fraction(value: Json) -> Result<Fraction, String> {
    let mut fields = object(value)?;
    let fraction = Fraction { numerator: number(&mut fields, "numerator")?,
        denominator: number(&mut fields, "denominator")? };
    end(fields)?;
    if fraction.denominator == 0 || fraction.numerator > fraction.denominator {
        return Err("invalid Andon evaluation fraction".into());
    }
    Ok(fraction)
}
pub(super) fn windows(value: Json) -> Result<Vec<AndonBudget>, String> {
    let Json::Array(values) = value else { return Err("windows must be an array".into()); };
    if values.len() > MAX_WINDOWS { return Err("too many Andon windows".into()); }
    values.into_iter().map(|value| {
        let mut fields = object(value)?;
        let budget = AndonBudget { window: number(&mut fields, "window")?,
            start: number(&mut fields, "start")?, end: number(&mut fields, "end")?,
            false_stops_allowed: number(&mut fields, "false_stops_allowed")?,
            cost_per_stop: number(&mut fields, "cost_per_stop")? };
        end(fields)?;
        Ok(budget)
    }).collect()
}

fn joint_policy(value: Json) -> Result<JointPromotionPolicy, String> {
    let mut fields = object(value)?;
    let id = number(&mut fields, "id")?;
    let generation = number(&mut fields, "generation")?;
    let escape = fraction(take(&mut fields, "maximum_escape_rate")?)?;
    let benign = fraction(take(&mut fields, "maximum_benign_stop_rate")?)?;
    let mut work = object(take(&mut fields, "budget")?)?;
    let budget = JointReplayBudget {
        cases: usize::try_from(number(&mut work, "cases")?).map_err(debug)?,
        member_outcomes: usize::try_from(number(&mut work, "member_outcomes")?).map_err(debug)?,
    };
    end(work)?; end(fields)?;
    JointPromotionPolicy::new(id, generation, escape, benign, budget).map_err(debug)
}
