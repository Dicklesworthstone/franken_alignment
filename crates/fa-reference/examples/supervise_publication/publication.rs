//! Explicit original witness requirements for the runnable supervisor. This is
//! orchestration of the native publication gate, not another validation engine.
mod native;
mod feed;
mod wait;
mod joint;
#[cfg(test)]
mod joint_tests;
#[cfg(test)]
mod waiting_tests;
use wait::{WaitBudget, WaitPolicy};
#[cfg(test)]
mod producer_tests;
#[cfg(test)]
mod bootstrap_tests;
use feed::FeedProfile;
use super::config::{debug, read_regular};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileHumanReviewer, FileOversight, FileOversightProfile};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::Error;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileSupervisedDriver};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::PublicationInputFile;
use fa_reference::action::consequence::delivery::publication_gate::{PublicationLimits, MAX_PUBLICATION_BINDINGS};
use fa_reference::action::consequence::oversight::evidence_source::FileEvidenceSource;
use fa_reference::action::{ElapsedTick, Purpose, Scope};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::PublicationProducerProfile;
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
    sources: PublicationSources,
    requests: Vec<WitnessRequest>,
    feed: Option<FeedProfile>,
    wait: Option<WaitPolicy>,
    snapshot_fallback: bool,
    joint: Option<joint::HeldOutJointPolicy>,
}
#[derive(Debug)]
enum PublicationSources {
    Captures { original: PublicationInputFile, current: PublicationInputFile },
    Producer { path: PathBuf, profile: PublicationProducerProfile },
}

/// Only reader bindings survive preparation, never cached current evidence or
/// authority. Every driver step reopens the selected native source.
pub struct PreparedPublication<'a> {
    current: CurrentPublication<'a>,
    feed: Option<&'a FeedProfile>,
    wait: Option<WaitBudget>,
}
enum CurrentPublication<'a> {
    Capture(&'a PublicationInputFile),
    Producer(PublicationInputFile),
}
impl CurrentPublication<'_> {
    fn reader(&self) -> &PublicationInputFile {
        match self { Self::Capture(reader) => reader, Self::Producer(reader) => reader }
    }
}

impl PublicationProfile {
    pub fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, PROFILE_BYTES)?)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let (json, joint) = joint::decode(bytes)?;
        let mut root = Fields::new(json)?;
        let schema = root.text("schema")?;
        let whole_input = matches!(schema.as_str(), "fa.supervised-whole-input/1" | "fa.supervised-whole-input/2");
        let snapshot_fallback = matches!(schema.as_str(), "fa.supervised-witnesses/5" | "fa.supervised-whole-input/2");
        if snapshot_fallback && root.text("history")? != "exact_current_snapshot" {
            return Err("snapshot profiles require history=exact_current_snapshot".into());
        }
        let wait = if schema == "fa.supervised-witnesses/4" {
            Some(WaitPolicy::new(root.number("max_retries")?)?)
        } else { None };
        let source = root.number("source")?;
        let (sources, feed) = match schema.as_str() {
            "fa.supervised-witnesses/1" | "fa.supervised-witnesses/2" | "fa.supervised-witnesses/4" => {
                let original = PublicationInputFile::new(path(root.text("original")?)?, source).map_err(debug)?;
                let current = PublicationInputFile::new(path(root.text("current")?)?, source).map_err(debug)?;
                let feed = if schema != "fa.supervised-witnesses/1" {
                    Some(FeedProfile::decode(root.take("feed")?)?)
                } else { None };
                (PublicationSources::Captures { original, current }, feed)
            }
            "fa.supervised-witnesses/3" | "fa.supervised-witnesses/5"
            | "fa.supervised-whole-input/1" | "fa.supervised-whole-input/2" => {
                let mut producer = Fields::new(root.take("producer")?)?;
                let path = path(producer.text("path")?)?;
                let mut scope = Fields::new(producer.take("scope")?)?;
                let scope_value = Scope { tenant: scope.number("tenant")?, principal: scope.number("principal")?,
                    run: scope.number("run")?, branch: scope.number("branch")?, authority: scope.number("authority")?,
                    purpose: Purpose::Effect };
                scope.end()?; producer.end()?;
                let (feed, profile) = FeedProfile::producer(root.take("feed")?, path.clone(), source, scope_value)?;
                (PublicationSources::Producer { path, profile }, Some(feed))
            }
            _ => return Err("unsupported supervised witness profile".into()),
        };
        let mut l = Fields::new(root.take("limits")?)?;
        let limits = PublicationLimits { bindings: usize::try_from(l.number("bindings")?).map_err(debug)?,
            validation: RefinementBudget { steps: l.number("steps")?, value_bytes: l.number("value_bytes")? } };
        l.end()?;
        if limits.bindings == 0 || limits.bindings > MAX_PUBLICATION_BINDINGS {
            return Err("publication binding limit is out of range".into());
        }
        let raw = root.take("requests")?;
        let raw = raw.as_array().ok_or("requests must be an array")?;
        // Existing schemas stay strict. Only explicit whole-input mode accepts
        // no structured queries; preparation must then retain an opaque witness.
        if raw.len() > MAX_WITNESSES || (!whole_input && raw.is_empty()) {
            return Err("a bounded nonempty witness recipe is required".into());
        }
        if whole_input && !raw.is_empty() {
            return Err("whole-input profiles require an explicit empty requests array".into());
        }
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
        Ok(Self { limits, sources, requests, feed, wait, snapshot_fallback, joint })
    }

    fn check_scope(&self, profile: &FileOversightProfile) -> Result<(), JournalError> {
        if let PublicationSources::Producer { profile: producer, .. } = &self.sources
            && producer.scope != profile.delivery.scope { return Err(Error::Binding.into()); }
        Ok(())
    }

    /// Pin every selected native gate in the first canonical image. Source-only
    /// profiles retain their original meaning; feed-backed modes never omit the feed.
    pub fn create(&self, directory: &Path, profile: FileOversightProfile)
        -> Result<(FileOversight, FileHumanReviewer), JournalError>
    {
        self.check_scope(&profile)?;
        if let Some(joint) = self.joint_policy() {
            return FileOversight::create_with_joint_publication(directory, profile, joint);
        }
        match &self.feed {
            Some(feed) if self.snapshot_fallback => FileOversight::create_with_publication_snapshot_fallback(directory,
                profile, self.limits, feed.changes, feed.freshness),
            Some(feed) => FileOversight::create_with_publication_change_freshness(directory,
                profile, self.limits, feed.changes, feed.freshness),
            None => FileOversight::create_with_publication_validation(directory, profile, self.limits),
        }
    }

    /// All feed-backed native policy fields are checked BEFORE the recovery fence.
    /// No feed/capture file is read. The source-only opener also refuses a stored
    /// stronger profile (after its native fence), never disabling the stored gate.
    pub fn open(&self, directory: &Path, profile: FileOversightProfile)
        -> Result<(FileOversight, FileHumanReviewer), JournalError>
    {
        self.check_scope(&profile)?;
        if let Some(joint) = self.joint_policy() {
            return FileOversight::open_with_joint_publication(directory, profile, joint);
        }
        match &self.feed {
            Some(feed) if self.snapshot_fallback => FileOversight::open_with_publication_snapshot_fallback(directory,
                profile, self.limits, feed.changes, feed.freshness),
            Some(feed) => FileOversight::open_with_publication_change_freshness(directory,
                profile, self.limits, feed.changes, feed.freshness),
            None => {
                let result = FileOversight::open_with_publication_validation(directory, profile, self.limits)?;
                match result.0.publication_change_status() {
                    Err(JournalError::Contract(Error::Incomplete)) => Ok(result),
                    Ok(_) => Err(Error::Binding.into()),
                    Err(error) => Err(error),
                }
            }
        }
    }

    // Preparation on an already owned host must not silently use a different
    // history policy. Refuse before reading, withdrawing, or binding evidence.
    fn check_preparation(&self, host: &FileOversight) -> Result<(), String> {
        if let Some(joint) = self.joint_policy() {
            host.check_joint_publication_profile(joint).map_err(debug)?;
        }
        if host.publication_validation_profile().map_err(debug)? != Some(self.limits) {
            return Err("stored publication limits differ from the explicit profile".into());
        }
        if host.publication_snapshot_fallback_enabled().map_err(debug)? != self.snapshot_fallback {
            return Err("stored publication history policy differs from the explicit profile".into());
        }
        Ok(())
    }

    /// Runnable preparation includes feed catch-up from the SAME original
    /// producer image. Obtain a real clock after reading, not a saved journal
    /// tick. Raw capture profiles keep their original acquisition/clock behavior.
    pub fn prepare_with_clock<F>(&self, host: &mut FileOversight, attempt: u64, clock: F)
        -> Result<PreparedPublication<'_>, String>
    where F: FnMut() -> ElapsedTick {
        let PublicationSources::Producer { path, profile } = &self.sources else {
            return self.prepare(host, attempt);
        };
        self.check_preparation(host)?;
        let feed = self.feed.as_ref().ok_or("producer preparation requires the configured feed")?;
        let reader = host.publication_producer_reader(attempt, path, *profile).map_err(debug)?;
        // Profile parsing fixes the recipe; native binding checks the actual
        // original image (including mandatory opaque input for an empty recipe),
        // stages catch-up first, and never installs this image as fresh evidence.
        host.bind_publication_from_producer(host.revision(), attempt, &reader, &feed.reader,
            self.requests.clone(), clock).map_err(debug)?.map_err(debug)?;
        Ok(PreparedPublication { current: CurrentPublication::Producer(reader), feed: Some(feed), wait: None })
    }

    /// Low-level preparation requires an already matching feed cut. Running
    /// workflows use prepare_with_clock so an advanced producer is caught up.
    /// Capture the original image BEFORE helper launch and bind it through the
    /// actual owner. Producer mode obtains the gateway's complete frozen action;
    /// it never guesses an attempt, rebuilds an action or needs a shadow journal.
    pub fn prepare(&self, host: &mut FileOversight, attempt: u64) -> Result<PreparedPublication<'_>, String> {
        self.check_preparation(host)?;
        let (original, current) = match &self.sources {
            PublicationSources::Captures { original, current } => {
                (original.read_capture().map_err(debug)?, CurrentPublication::Capture(current))
            }
            PublicationSources::Producer { path, profile } => {
                let reader = host.publication_producer_reader(attempt, path, *profile).map_err(debug)?;
                let original = reader.read_capture().map_err(debug)?;
                (original, CurrentPublication::Producer(reader))
            }
        };
        if self.wait.is_some() && original.input_cut().is_none() {
            return Err("producer waiting requires an original cut-bound capture".into());
        }
        if self.requests.is_empty() {
            if original.inputs().opaque().is_none() {
                return Err("whole-input supervision requires the original opaque input view".into());
            }
        } else if original.inputs().structured().is_none() {
            return Err("checked supervision requires the original structured witness image".into());
        }
        // Bind the COMPLETE original packet, including every present lane. No
        // empty recipe, helper explanation or later capture may narrow its view.
        if original.identity().source != current.reader().source() { return Err("original witness producer mismatch".into()); }
        host.bind_publication_file_source(host.revision(), attempt, original, self.requests.clone()).map_err(debug)?;
        Ok(PreparedPublication { current, feed: self.feed.as_ref(), wait: self.wait.map(WaitBudget::new) })
    }
}

impl PreparedPublication<'_> {
    /// The SAME native policy-source reader remains mandatory. Witness bytes are
    /// acquired separately at authorize, dispatch and first publication; neither
    /// a cached result nor human approval can replace a missing current capture.
    pub fn step<F>(&self, driver: &mut FileSupervisedDriver, evidence: &mut FileEvidenceSource,
        time: &mut F, human: Option<&FileHumanPermit>) -> Result<FileDriverEvent, String>
    where F: FnMut() -> ElapsedTick {
        match self.feed {
            Some(feed) => driver.step_from_files_with_publication_feed(evidence, self.current.reader(),
                &feed.reader, time, human, None).publication.evidence.result.map_err(debug),
            None => driver.step_from_files_with_publication_source(evidence, self.current.reader(), time, human, None)
                .evidence.result.map_err(debug),
        }
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
