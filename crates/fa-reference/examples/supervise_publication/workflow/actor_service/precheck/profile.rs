//! Public opt-in for one exact-predicate sidecar, not the private Config.
use super::super::{Config, PeerProfile, Profile};
use crate::config::{debug, read_regular};
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::cache::PrecheckCacheLimits;
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::MAX_PRECHECK_EXCHANGES;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_PRECHECK_QUERIES;
use fa_reference::action::consequence::oversight::actor_peer::MAX_PEER_CONNECTIONS;
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Identity, audience and poll interval come from the separately supplied actor
/// profile. Nothing in a request can select these limits or issue a disclosure.
#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub(super) socket: PathBuf,
    pub(super) expected_policy_epoch: u64,
    pub(super) disclosure_id: u64,
    pub(super) query_limit: usize,
    pub(super) exchange_limit: usize,
    pub(super) candidate_limit: u64,
    pub(super) connection_limit: u64,
    pub(super) runtime_ms: u64,
    pub(super) cache: Option<PrecheckCacheLimits>,
    pub(super) full_policy: bool,
}

impl Settings {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, 16_384)?)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut fields = Fields::new(strict_json::parse(bytes, Limits {
            max_bytes: 16_384,
            max_depth: 3,
            max_items: 64,
            max_string_bytes: 4_096,
        }).map_err(debug)?)?;
        let (cached, full_policy) = match fields.text("schema")?.as_str() {
            "fa.actor-precheck-service/1" => (false, false),
            "fa.actor-precheck-service/2" => (true, false),
            "fa.actor-precheck-service/3" => (true, true),
            _ => return Err("unsupported precheck service schema".into()),
        };
        let disclosure = if full_policy { "full" } else { "exact_predicates" };
        if fields.text("policy_disclosure")? != disclosure {
            return Err("policy disclosure does not match the selected service schema".into());
        }
        // Full literals are an explicit operator choice, not a default or a
        // request-selected escalation. The declared leakage cannot be softened.
        if full_policy {
            let mut leakage = Fields::new(fields.take("leakage")?)?;
            for (key, expected) in [
                ("policy_structure", "disclosed"), ("policy_literals", "disclosed"),
                ("source_values", "withheld"), ("helper_feedback", "withheld"),
                ("consequences", "withheld"), ("timing", "observable"),
                ("query_usage", "observable"),
            ] {
                if leakage.text(key)? != expected {
                    return Err("full-policy leakage declaration does not match the native boundary".into());
                }
            }
            leakage.end()?;
        }
        let socket = PathBuf::from(fields.text("socket")?);
        if !socket.is_absolute() || socket.file_name().is_none()
            || socket.components().any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
            || socket.as_os_str().as_encoded_bytes().contains(&0)
            || socket.as_os_str().as_encoded_bytes().len() > 100
        {
            return Err("precheck socket must be an absolute normalized path of at most 100 bytes".into());
        }
        let expected_policy_epoch = fields.number("expected_policy_epoch")?;
        let disclosure_id = fields.number("disclosure_id")?;
        let query_limit = usize::try_from(fields.number("query_limit")?).map_err(debug)?;
        let exchange_limit = usize::try_from(fields.number("exchange_limit")?).map_err(debug)?;
        let candidate_limit = fields.number("candidate_limit")?;
        let connection_limit = fields.number("connection_limit")?;
        let runtime_ms = fields.number("runtime_ms")?;
        // Cache selection is explicit and closed: v1 never consumes v2 fields,
        // and v2 never substitutes defaults for missing or invalid limits.
        let cache = if cached {
            let limits = PrecheckCacheLimits {
                entries: usize::try_from(fields.number("cache_entries")?).map_err(debug)?,
                retained_bytes: usize::try_from(fields.number("cache_bytes")?).map_err(debug)?,
            };
            limits.validate().map_err(debug)?;
            Some(limits)
        } else { None };
        fields.end()?;
        if disclosure_id == 0 || query_limit == 0 || query_limit > MAX_PRECHECK_QUERIES
            || exchange_limit == 0 || exchange_limit > MAX_PRECHECK_EXCHANGES
            || candidate_limit == 0 || candidate_limit > MAX_PEER_CONNECTIONS
            || connection_limit == 0 || connection_limit > candidate_limit
            || runtime_ms == 0 || runtime_ms > 3_600_000
        {
            return Err("precheck disclosure, work or lifetime limit is out of range".into());
        }
        Ok(Self { socket, expected_policy_epoch, disclosure_id, query_limit,
            exchange_limit, candidate_limit, connection_limit, runtime_ms, cache, full_policy })
    }

    /// Public actor-side checks, with no private Config, source or journal access.
    pub(crate) fn check_actor(&self, actor: &Profile) -> Result<(), String> {
        if self.full_policy && self.cache.is_none() {
            return Err("full-policy service requires explicit cache limits".into());
        }
        if let Some(limits) = self.cache { limits.validate().map_err(debug)?; }
        if self.socket == actor.socket {
            return Err("precheck and ordinary actor sockets must be distinct".into());
        }
        if actor.poll_ms > self.runtime_ms.min(actor.runtime_ms) {
            return Err("precheck lifetime is shorter than the actor poll interval".into());
        }
        self.socket_profile(actor).check_directory()
    }

    /// Called before owner creation and again before the explicit bootstrap.
    /// A pre-existing path is never removed to make a disclosure service start.
    pub(crate) fn check_host(&self, config: &Config, actor: &Profile, reviewer: &PeerProfile)
        -> Result<(), String>
    {
        actor.check_host(config)?;
        reviewer.check_host(config)?;
        self.check_actor(actor)?;
        if self.socket == reviewer.socket(actor.request)
            || self.socket == super::super::super::control::socket_path(reviewer, actor.request)
        {
            return Err("precheck, reviewer and stop sockets must be distinct".into());
        }
        match fs::symlink_metadata(&self.socket) {
            Ok(_) => Err("precheck socket path already exists; no replacement is permitted".into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(debug(error)),
        }
    }

    pub(super) fn socket_profile(&self, actor: &Profile) -> Profile {
        let mut selected = actor.clone();
        selected.socket = self.socket.clone();
        selected
    }
}

struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> {
        match value { Json::Object(fields) => Ok(Self(fields)), _ => Err("expected precheck profile object".into()) }
    }
    fn take(&mut self, name: &str) -> Result<Json, String> {
        self.0.remove(name).ok_or_else(|| format!("missing precheck field {name}"))
    }
    fn text(&mut self, name: &str) -> Result<String, String> {
        self.take(name)?.as_str().map(str::to_owned).ok_or_else(|| format!("{name} must be text"))
    }
    fn number(&mut self, name: &str) -> Result<u64, String> {
        self.take(name)?.as_u64().ok_or_else(|| format!("{name} must be an unsigned integer"))
    }
    fn end(self) -> Result<(), String> {
        if self.0.is_empty() { Ok(()) } else { Err("unknown precheck service field".into()) }
    }
}
