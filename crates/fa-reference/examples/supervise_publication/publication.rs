//! Explicit original witness requirements for the runnable supervisor. This is
//! orchestration of the native publication gate, not another validation engine.
use super::config::{debug, read_regular};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileSupervisedDriver};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{FilePublicationCapture, PublicationInputFile};
use fa_reference::action::consequence::delivery::publication_gate::{PublicationLimits, MAX_PUBLICATION_BINDINGS};
use fa_reference::action::consequence::oversight::evidence_source::FileEvidenceSource;
use fa_reference::action::ElapsedTick;
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::witness::{QueryRole, WitnessRequest, MAX_WITNESSES};
use fa_reference::witness::refinement::RefinementBudget;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const PROFILE_BYTES: usize = 65_536;

/// Operator-selected original and live sources are separate. Neither is read by
/// parsing or receipt-only recovery. Requirements cannot be chosen from a later
/// image after the helper's answer is known.
#[derive(Debug)]
pub struct PublicationProfile {
    pub limits: PublicationLimits,
    original: PublicationInputFile,
    current: PublicationInputFile,
    requests: Vec<WitnessRequest>,
}
impl PublicationProfile {
    pub fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, PROFILE_BYTES)?)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let json = strict_json::parse(bytes, Limits { max_bytes: PROFILE_BYTES, max_depth: 5,
            max_items: 2048, max_string_bytes: 4096 }).map_err(debug)?;
        let mut root = Fields::new(json)?;
        if root.text("schema")? != "fa.supervised-witnesses/1" {
            return Err("unsupported supervised witness profile".into());
        }
        let source = root.number("source")?;
        let original = path(root.text("original")?)?;
        let current = path(root.text("current")?)?;
        let mut l = Fields::new(root.take("limits")?)?;
        let limits = PublicationLimits { bindings: usize::try_from(l.number("bindings")?).map_err(debug)?,
            validation: RefinementBudget { steps: l.number("steps")?, value_bytes: l.number("value_bytes")? } };
        l.end()?;
        if limits.bindings == 0 || limits.bindings > MAX_PUBLICATION_BINDINGS {
            return Err("publication binding limit is out of range".into());
        }
        let raw = root.take("requests")?;
        let raw = raw.as_array().ok_or("requests must be an array")?;
        if raw.is_empty() || raw.len() > MAX_WITNESSES { return Err("a bounded nonempty witness recipe is required".into()); }
        let mut requests = Vec::new();
        let mut identities = BTreeSet::new();
        for item in raw {
            let mut f = Fields::new(item.clone())?;
            let kind = f.text("kind")?;
            let (request, identity) = match kind.as_str() {
                "exact_value" => {
                    let key = f.number("key")?;
                    let role = match f.text("role")?.as_str() {
                        "subject" => QueryRole::Subject,
                        "policy_input" => QueryRole::PolicyInput,
                        "predicate_input" => QueryRole::PredicateInput,
                        _ => return Err("unknown exact-value query role".into()),
                    };
                    (WitnessRequest::ExactValue { key, role }, (0, key, 0))
                }
                "absent_key" => {
                    let key = f.number("key")?;
                    (WitnessRequest::AbsentKey { key }, (0, key, 0))
                }
                "empty_range" | "range_members" => {
                    let start = f.number("start")?; let end = f.number("end")?;
                    if start >= end { return Err("witness intervals must be nonempty half-open ranges".into()); }
                    let request = if kind == "empty_range" { WitnessRequest::EmptyRange { start, end } }
                        else { WitnessRequest::RangeMembers { start, end } };
                    (request, (1, start, end))
                }
                _ => return Err("unknown witness request kind".into()),
            };
            f.end()?;
            if !identities.insert(identity) { return Err("duplicate witness identity".into()); }
            requests.push(request);
        }
        root.end()?;
        Ok(Self { limits, original: PublicationInputFile::new(original, source).map_err(debug)?,
            current: PublicationInputFile::new(current, source).map_err(debug)?, requests })
    }

    /// Read once BEFORE launching any helper. Hold the exact original image in
    /// this local value until the original host binds the complete frozen action.
    /// A live file is not a fallback for a missing or incompatible original.
    pub fn original(&self) -> Result<FilePublicationCapture, String> {
        let original = self.original.read_capture().map_err(debug)?;
        if original.inputs().structured().is_none() {
            return Err("checked supervision requires the original structured witness image".into());
        }
        Ok(original)
    }
    pub fn bind(&self, host: &mut FileOversight, attempt: u64,
        original: FilePublicationCapture) -> Result<(), String>
    {
        if host.publication_validation_profile().map_err(debug)? != Some(self.limits) {
            return Err("stored publication limits differ from the explicit profile".into());
        }
        if original.identity().source != self.current.source() { return Err("original witness producer mismatch".into()); }
        host.bind_publication_file_source(host.revision(), attempt, original, self.requests.clone()).map_err(debug)
    }

    /// The SAME native policy-source reader remains mandatory. Witness bytes are
    /// acquired separately at authorize, dispatch and first publication; neither
    /// a cached result nor human approval can replace a missing current capture.
    pub fn step<F>(&self, driver: &mut FileSupervisedDriver, evidence: &mut FileEvidenceSource,
        time: &mut F, human: Option<&FileHumanPermit>) -> Result<FileDriverEvent, String>
    where F: FnMut() -> ElapsedTick {
        driver.step_from_files_with_publication_source(evidence, &self.current, time, human, None)
            .evidence.result.map_err(debug)
    }
}

fn path(text: String) -> Result<PathBuf, String> {
    let path = PathBuf::from(text);
    if !path.is_absolute() { return Err("publication paths must be absolute".into()); }
    Ok(path)
}
struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> {
        match value { Json::Object(fields) => Ok(Self(fields)), _ => Err("expected a profile object".into()) }
    }
    fn take(&mut self, field: &str) -> Result<Json, String> {
        self.0.remove(field).ok_or_else(|| format!("missing publication field {field}"))
    }
    fn text(&mut self, field: &str) -> Result<String, String> {
        self.take(field)?.as_str().map(str::to_owned).ok_or_else(|| format!("{field} must be text"))
    }
    fn number(&mut self, field: &str) -> Result<u64, String> {
        self.take(field)?.as_u64().ok_or_else(|| format!("{field} must be an unsigned integer"))
    }
    fn end(self) -> Result<(), String> {
        if self.0.is_empty() { Ok(()) } else { Err("unknown publication profile field".into()) }
    }
}
