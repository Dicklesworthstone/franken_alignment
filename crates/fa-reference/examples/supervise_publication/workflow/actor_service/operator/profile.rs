//! Explicit operator identity and disclosure; never selected by actor bytes.
use super::super::{Config, PeerProfile, Profile};
use crate::config::{debug, read_regular};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    MAX_EXPLANATION_EVENTS, MAX_EXPLANATION_QUERIES, MAX_INSPECTION_QUERIES,
    MAX_POLICY_REPLAY_PAGE_QUERIES, MAX_POLICY_REPLAY_QUERIES, MAX_REVIEW_EXPLANATION_EVENTS,
    MAX_REVIEW_EXPLANATION_QUERIES,
};
use fa_reference::action::consequence::oversight::actor_peer::{MAX_PEER_CONNECTIONS, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_wire::MAX_CHANNEL_EXCHANGES;
use fa_reference::action::consequence::policy_campaign::{
    MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES, ReplayLimits,
};
use fa_reference::action::{Purpose, Scope};
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::tower::publication::PublicationLimits;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub(crate) scope: Scope,
    pub(crate) socket: PathBuf,
    pub(crate) supervisor: PeerPolicy,
    pub(crate) operator: PeerPolicy,
    pub(crate) candidate_limit: u64,
    pub(crate) connection_limit: u64,
    pub(crate) exchange_limit: u64,
    pub(crate) response_bytes: usize,
    pub(crate) runtime_ms: u64,
    pub(crate) poll_ms: u64,
    pub(crate) inspection_queries: usize,
    pub(crate) explanation_queries: usize,
    pub(crate) explanation_replay_events: usize,
    pub(crate) explanation_disclosure: String,
    pub(crate) review_explanation: Option<ReviewExplanationSettings>,
    pub(crate) policy_replay: Option<PolicyReplaySettings>,
}

#[derive(Clone, Debug)]
pub(crate) struct ReviewExplanationSettings {
    pub(crate) queries: usize,
    pub(crate) max_replay_events: usize,
    pub(crate) disclosure: String,
}

impl ReviewExplanationSettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            queries: usize::try_from(fields.number("queries")?).map_err(debug)?,
            max_replay_events: usize::try_from(fields.number("max_replay_events")?)
                .map_err(debug)?,
            disclosure: fields.text("disclosure")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.queries == 0
            || self.queries > MAX_REVIEW_EXPLANATION_QUERIES
            || self.max_replay_events == 0
            || self.max_replay_events > MAX_REVIEW_EXPLANATION_EVENTS
            || self.disclosure != "policy_and_congress_trace"
        {
            return Err("review explanation requires explicit policy-and-congress-trace disclosure and bounded replay".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PolicyReplaySettings {
    pub(crate) queries: usize,
    pub(crate) page_queries: usize,
    pub(crate) limits: ReplayLimits,
    pub(crate) disclosure: String,
}

impl PolicyReplaySettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            queries: usize::try_from(fields.number("queries")?).map_err(debug)?,
            page_queries: match fields.optional_number("page_queries")? {
                None => 0,
                Some(0) => {
                    return Err("explicit policy replay page_queries must be positive".into());
                }
                Some(value) => usize::try_from(value).map_err(debug)?,
            },
            limits: ReplayLimits {
                cases: usize::try_from(fields.number("cases")?).map_err(debug)?,
                input_bytes: usize::try_from(fields.number("input_bytes")?).map_err(debug)?,
            },
            disclosure: fields.text("disclosure")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.queries == 0
            || self.queries > MAX_POLICY_REPLAY_QUERIES
            || self.page_queries > MAX_POLICY_REPLAY_PAGE_QUERIES
            || self.limits.cases == 0
            || self.limits.cases > MAX_REPLAY_CASES
            || self.limits.input_bytes == 0
            || self.limits.input_bytes > MAX_REPLAY_INPUT_BYTES
            || self.disclosure != "policy_and_case_traces"
        {
            return Err("policy replay requires explicit policy-and-case-traces disclosure and bounded work".into());
        }
        Ok(())
    }
}

impl Settings {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, 16_384)?)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut fields = Fields::new(
            strict_json::parse(
                bytes,
                Limits {
                    max_bytes: 16_384,
                    max_depth: 4,
                    max_items: 128,
                    max_string_bytes: 4_096,
                },
            )
            .map_err(debug)?,
        )?;
        if fields.text("schema")? != "fa.tower-operator-service/1" {
            return Err("unsupported Tower operator service schema".into());
        }
        let mut audience = Fields::new(fields.take("scope")?)?;
        let scope = Scope {
            tenant: audience.number("tenant")?,
            principal: audience.number("principal")?,
            run: audience.number("run")?,
            branch: audience.number("branch")?,
            authority: audience.number("authority")?,
            purpose: Purpose::Effect,
        };
        audience.end()?;
        let settings = Self {
            scope,
            socket: PathBuf::from(fields.text("socket")?),
            supervisor: identity(fields.take("supervisor")?)?,
            operator: identity(fields.take("operator")?)?,
            candidate_limit: fields.number("candidate_limit")?,
            connection_limit: fields.number("connection_limit")?,
            exchange_limit: fields.number("exchange_limit")?,
            response_bytes: fields
                .optional_number("response_bytes")?
                .map(usize::try_from)
                .transpose()
                .map_err(debug)?
                .unwrap_or_else(|| PublicationLimits::default().max_response_bytes),
            runtime_ms: fields.number("runtime_ms")?,
            poll_ms: fields.number("poll_ms")?,
            inspection_queries: usize::try_from(fields.number("inspection_queries")?)
                .map_err(debug)?,
            explanation_queries: usize::try_from(fields.number("explanation_queries")?)
                .map_err(debug)?,
            explanation_replay_events: usize::try_from(fields.number("explanation_replay_events")?)
                .map_err(debug)?,
            explanation_disclosure: fields.text("explanation_disclosure")?,
            review_explanation: match fields.optional("review_explanation") {
                None | Some(Json::Null) => None,
                Some(value) => Some(ReviewExplanationSettings::decode(value)?),
            },
            policy_replay: match fields.optional("policy_replay") {
                None | Some(Json::Null) => None,
                Some(value) => Some(PolicyReplaySettings::decode(value)?),
            },
        };
        fields.end()?;
        settings.validate()?;
        Ok(settings)
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        if !(PublicationLimits {
            max_response_bytes: self.response_bytes,
        })
        .valid()
        {
            return Err(
                "operator response_bytes exceeds the native publication response bounds".into(),
            );
        }
        if self.scope.purpose != Purpose::Effect
            || [
                self.scope.tenant,
                self.scope.principal,
                self.scope.run,
                self.scope.branch,
                self.scope.authority,
            ]
            .contains(&0)
        {
            return Err("operator audience identifiers must be nonzero in the effect scope".into());
        }
        let normalized: PathBuf = self.socket.components().collect();
        if !self.socket.is_absolute()
            || self.socket.file_name().is_none()
            || normalized.as_os_str() != self.socket.as_os_str()
            || self
                .socket
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
            || self.socket.as_os_str().as_encoded_bytes().contains(&0)
            || self.socket.as_os_str().as_encoded_bytes().len() > 100
        {
            return Err(
                "operator socket must be an absolute normalized path of at most 100 bytes".into(),
            );
        }
        if self.candidate_limit == 0
            || self.candidate_limit > MAX_PEER_CONNECTIONS
            || self.connection_limit == 0
            || self.connection_limit > self.candidate_limit
            || self.exchange_limit == 0
            || self.exchange_limit > MAX_CHANNEL_EXCHANGES
            || self.runtime_ms == 0
            || self.runtime_ms > 3_600_000
            || self.poll_ms == 0
            || self.poll_ms > 1_000
            || self.poll_ms > self.runtime_ms
            || self.inspection_queries > MAX_INSPECTION_QUERIES
            || self.explanation_queries > MAX_EXPLANATION_QUERIES
            || (self.inspection_queries == 0
                && self.explanation_queries == 0
                && self.review_explanation.is_none()
                && self.policy_replay.is_none())
        {
            return Err("operator service work or lifetime limit is out of range".into());
        }
        if self.explanation_queries == 0 {
            if self.explanation_replay_events != 0 || self.explanation_disclosure != "none" {
                return Err(
                    "disabled explanation requires no replay work and disclosure none".into(),
                );
            }
        } else if self.explanation_replay_events == 0
            || self.explanation_replay_events > MAX_EXPLANATION_EVENTS
            || self.explanation_disclosure != "policy_and_selected_witnesses"
        {
            return Err("explanation requires explicit policy-and-selected-witness disclosure and bounded replay".into());
        }
        if let Some(selected) = &self.policy_replay {
            selected.validate()?;
        }
        if let Some(selected) = &self.review_explanation {
            selected.validate()?;
        }
        Ok(())
    }

    /// Audience and declared server identity must match the original service.
    /// The operator identity is independently selected, never inherited from
    /// the actor or reviewer and never widened by connecting to another socket.
    pub(super) fn check_actor(&self, actor: &Profile) -> Result<(), String> {
        self.validate()?;
        if self.scope != actor.scope || self.supervisor != actor.supervisor {
            return Err(
                "operator audience/server identity does not match the original actor service"
                    .into(),
            );
        }
        if self.socket == actor.socket {
            return Err("operator and actor sockets must be distinct".into());
        }
        self.check_directory()
    }

    /// Preflight before source, time or persistent setup. Existing files,
    /// sockets and symlinks are never replaced to make the sidecar start.
    pub(crate) fn check_host(
        &self,
        config: &Config,
        actor: &Profile,
        reviewer: &PeerProfile,
        additional_sockets: &[&Path],
    ) -> Result<(), String> {
        actor.check_host(config)?;
        reviewer.check_host(config)?;
        self.check_actor(actor)?;
        if self.scope != config.profile.delivery.scope
            || self.socket == reviewer.socket(actor.request)
            || self.socket == super::super::super::control::socket_path(reviewer, actor.request)
            || additional_sockets
                .iter()
                .any(|path| self.socket.as_path() == *path)
        {
            return Err("operator audience or socket conflicts with the original service".into());
        }
        self.check_unbound()
    }

    pub(super) fn check_unbound(&self) -> Result<(), String> {
        match fs::symlink_metadata(&self.socket) {
            Ok(_) => Err("operator socket path already exists; no replacement is permitted".into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(debug(error)),
        }
    }

    /// Public client-side check: no private Config, source or journal is read.
    pub(crate) fn check_directory(&self) -> Result<(), String> {
        self.validate()?;
        let metadata = fs::symlink_metadata(
            self.socket
                .parent()
                .ok_or("missing operator socket directory")?,
        )
        .map_err(debug)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.mode() & 0o022 != 0
            || metadata.uid() != self.supervisor.uid()
            || metadata.gid() != self.supervisor.gid()
        {
            return Err(
                "operator socket directory must be supervisor-owned and not group/other writable"
                    .into(),
            );
        }
        Ok(())
    }

    pub(crate) fn secure_socket(&self) -> Result<(), String> {
        let metadata = fs::symlink_metadata(&self.socket).map_err(debug)?;
        if !metadata.file_type().is_socket()
            || metadata.uid() != self.supervisor.uid()
            || metadata.gid() != self.supervisor.gid()
        {
            return Err("operator socket has unexpected ownership or type".into());
        }
        fs::set_permissions(&self.socket, fs::Permissions::from_mode(0o660)).map_err(debug)
    }
}

fn identity(value: Json) -> Result<PeerPolicy, String> {
    let mut fields = Fields::new(value)?;
    let uid = u32::try_from(fields.number("uid")?).map_err(debug)?;
    let gid = u32::try_from(fields.number("gid")?).map_err(debug)?;
    let pid = match fields.take("pid")? {
        Json::Null => None,
        value => Some(
            u32::try_from(
                value
                    .as_u64()
                    .ok_or("pid must be null or an unsigned integer")?,
            )
            .map_err(debug)?,
        ),
    };
    fields.end()?;
    PeerPolicy::new(uid, gid, pid).map_err(debug)
}

struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> {
        match value {
            Json::Object(fields) => Ok(Self(fields)),
            _ => Err("expected operator profile object".into()),
        }
    }
    fn take(&mut self, name: &str) -> Result<Json, String> {
        self.0
            .remove(name)
            .ok_or_else(|| format!("missing operator field {name}"))
    }
    fn optional(&mut self, name: &str) -> Option<Json> {
        self.0.remove(name)
    }
    fn text(&mut self, name: &str) -> Result<String, String> {
        self.take(name)?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{name} must be text"))
    }
    fn number(&mut self, name: &str) -> Result<u64, String> {
        self.take(name)?
            .as_u64()
            .ok_or_else(|| format!("{name} must be an unsigned integer"))
    }
    fn optional_number(&mut self, name: &str) -> Result<Option<u64>, String> {
        self.optional(name)
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| format!("{name} must be an unsigned integer"))
            })
            .transpose()
    }
    fn end(self) -> Result<(), String> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err("unknown operator service field".into())
        }
    }
}
