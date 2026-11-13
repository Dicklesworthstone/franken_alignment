//! Independently selected governor identity and bounded original campaign work.
use super::super::{Config, PeerProfile, Profile, operator};
use super::protocol::MAX_REPORT_BYTES;
use crate::config::{debug, read_regular};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::FileCampaignRequirement;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_POLICY_PROMOTION_REPLAY_EVENTS;
use fa_reference::action::consequence::oversight::actor_peer::{MAX_PEER_CONNECTIONS, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_wire::MAX_CHANNEL_EXCHANGES;
use fa_reference::action::consequence::oversight::policy_governance::MAX_POLICY_CAMPAIGNS;
use fa_reference::action::consequence::policy_campaign::{
    MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES, ReplayLimits,
};
use fa_reference::action::{Purpose, Scope};
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

pub(crate) const MAX_CAMPAIGNS: usize = 8;

/// Independent cumulative admission for native Request/Approve reconstruction.
/// Each entered transition also has a separately bounded complete-history cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NumericalReplaySettings {
    pub(crate) max_replay_events: usize,
    pub(crate) events: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub(crate) scope: Scope,
    pub(crate) socket: PathBuf,
    pub(crate) supervisor: PeerPolicy,
    pub(crate) governor: PeerPolicy,
    pub(crate) candidate_limit: u64,
    pub(crate) connection_limit: u64,
    pub(crate) exchange_limit: u64,
    pub(crate) runtime_ms: u64,
    pub(crate) poll_ms: u64,
    pub(crate) campaign_limit: usize,
    pub(crate) report_bytes: usize,
    pub(crate) numerical_replay: Option<NumericalReplaySettings>,
    campaigns: FileCampaignRequirement,
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
                    max_string_bytes: 4096,
                },
            )
            .map_err(debug)?,
        )?;
        if fields.text("schema")? != "fa.policy-governor-service/1" {
            return Err("unsupported policy governor service schema".into());
        }
        let mut selected = Fields::new(fields.take("scope")?)?;
        let scope = Scope {
            tenant: selected.number("tenant")?,
            principal: selected.number("principal")?,
            run: selected.number("run")?,
            branch: selected.number("branch")?,
            authority: selected.number("authority")?,
            purpose: Purpose::Effect,
        };
        selected.end()?;
        let socket = PathBuf::from(fields.text("socket")?);
        let supervisor = identity(fields.take("supervisor")?)?;
        let governor = identity(fields.take("governor")?)?;
        let candidate_limit = fields.number("candidate_limit")?;
        let connection_limit = fields.number("connection_limit")?;
        let exchange_limit = fields.number("exchange_limit")?;
        let runtime_ms = fields.number("runtime_ms")?;
        let poll_ms = fields.number("poll_ms")?;
        let campaign_limit = fields.size("campaign_limit")?;
        let report_bytes = fields.size("report_bytes")?;
        let numerical_replay = match fields.0.remove("numerical_replay") {
            None => None,
            Some(value) => {
                let mut numerical = Fields::new(value)?;
                let selected = NumericalReplaySettings {
                    max_replay_events: numerical.size("max_replay_events")?,
                    events: numerical.size("events")?,
                };
                numerical.end()?;
                Some(selected)
            }
        };
        let mut campaign = Fields::new(fields.take("campaigns")?)?;
        let campaigns = FileCampaignRequirement {
            limits: ReplayLimits {
                cases: campaign.size("cases")?,
                input_bytes: campaign.size("input_bytes")?,
            },
            max_campaigns: campaign.size("max_campaigns")?,
        };
        campaign.end()?;
        fields.end()?;
        let settings = Self {
            scope,
            socket,
            supervisor,
            governor,
            candidate_limit,
            connection_limit,
            exchange_limit,
            runtime_ms,
            poll_ms,
            campaign_limit,
            report_bytes,
            numerical_replay,
            campaigns,
        };
        settings.validate()?;
        Ok(settings)
    }

    fn validate(&self) -> Result<(), String> {
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
            return Err("governor audience must be a complete nonzero effect scope".into());
        }
        if !self.socket.is_absolute()
            || self.socket.file_name().is_none()
            || self
                .socket
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
            || self.socket.components().collect::<PathBuf>().as_os_str() != self.socket.as_os_str()
            || self.socket.as_os_str().as_encoded_bytes().contains(&0)
            || self.socket.as_os_str().as_encoded_bytes().len() > 100
        {
            return Err(
                "governor socket must be an absolute normalized path of at most 100 bytes".into(),
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
            || self.poll_ms > 1000
            || self.poll_ms > self.runtime_ms
        {
            return Err(
                "governor connection, exchange and lifetime limits are out of range".into(),
            );
        }
        if self.campaigns.limits.cases == 0
            || self.campaigns.limits.cases > MAX_REPLAY_CASES
            || self.campaigns.limits.input_bytes == 0
            || self.campaigns.limits.input_bytes > MAX_REPLAY_INPUT_BYTES
            || self.campaigns.max_campaigns == 0
            || self.campaigns.max_campaigns > MAX_POLICY_CAMPAIGNS
            || self.campaign_limit == 0
            || self.campaign_limit > MAX_CAMPAIGNS
            || self.campaign_limit > self.campaigns.max_campaigns
            || self.report_bytes == 0
            || self.report_bytes > MAX_REPORT_BYTES
        {
            return Err(
                "governor campaign work or report bounds exceed the original finite limits".into(),
            );
        }
        if let Some(selected) = self.numerical_replay {
            let maximum = usize::try_from(self.exchange_limit)
                .ok()
                .and_then(|count| count.checked_mul(selected.max_replay_events));
            if selected.max_replay_events == 0
                || selected.max_replay_events > MAX_POLICY_PROMOTION_REPLAY_EVENTS
                || selected.events == 0
                || maximum.is_none_or(|maximum| selected.events > maximum)
            {
                return Err("governor numerical replay requires finite per-transition and cumulative event allowances".into());
            }
        }
        Ok(())
    }

    pub(crate) fn campaign_requirement(&self) -> FileCampaignRequirement {
        self.campaigns
    }

    pub(super) fn check_actor(&self, actor: &Profile) -> Result<(), String> {
        self.validate()?;
        if self.scope != actor.scope
            || self.supervisor != actor.supervisor
            || !disjoint(self.governor, actor.actor)
        {
            return Err(
                "governor audience/server mismatch or overlapping actor credentials".into(),
            );
        }
        self.check_directory()
    }

    /// Complete role and pathname preflight before source, clock or Store work.
    /// Different socket names never establish role separation by themselves.
    pub(crate) fn check_host(
        &self,
        config: &Config,
        actor: &Profile,
        reviewer: &PeerProfile,
        operator: &operator::Settings,
        requests: &[u64],
    ) -> Result<(), String> {
        if self.numerical_replay.is_some() {
            return Err(
                "numerical governor work requires an explicitly governed learned sequence".into(),
            );
        }
        self.check_host_roles(config, actor, reviewer, operator, requests)
    }

    pub(crate) fn check_learned_host(
        &self,
        config: &Config,
        actor: &Profile,
        reviewer: &PeerProfile,
        operator: &operator::Settings,
        requests: &[u64],
    ) -> Result<(), String> {
        if self.numerical_replay.is_none() {
            return Err(
                "learned governor requires its own explicit numerical replay allowance".into(),
            );
        }
        operator.check_governed_learned_operator()?;
        self.check_host_roles(config, actor, reviewer, operator, requests)
    }

    fn check_host_roles(
        &self,
        config: &Config,
        actor: &Profile,
        reviewer: &PeerProfile,
        operator: &operator::Settings,
        requests: &[u64],
    ) -> Result<(), String> {
        actor.check_host(config)?;
        reviewer.check_host(config)?;
        self.check_actor(actor)?;
        if self.scope != config.profile.delivery.scope
            || self.scope != operator.scope
            || self.supervisor != operator.supervisor
            || self.supervisor != reviewer.supervisor_policy()?
            || operator.policy_promotion.is_none()
            || !disjoint(self.governor, operator.operator)
            || !disjoint(self.governor, reviewer.reviewer_policy()?)
        {
            return Err("governor requires the same server/audience and distinct actor, operator and reviewer credential sets".into());
        }
        let selected: BTreeSet<_> = requests.iter().copied().collect();
        if requests.len() < 2
            || selected.len() != requests.len()
            || selected.contains(&0)
            || !selected.contains(&actor.request)
        {
            return Err("governor requires an explicit nonzero resident request schedule".into());
        }
        let socket = resolved(&self.socket)?;
        let store = resolved(&config.store)?;
        if socket.starts_with(&store) {
            return Err("governor socket must be outside the authority store".into());
        }
        let mut other = vec![actor.socket.clone(), operator.socket.clone()];
        for request in requests {
            other.push(reviewer.socket(*request));
            other.push(super::super::super::control::socket_path(
                reviewer, *request,
            ));
        }
        for path in other {
            if socket == resolved(&path)? {
                return Err("governor socket conflicts with an actor, operator, scheduled reviewer or Stop endpoint".into());
            }
        }
        self.check_unbound()
    }

    pub(super) fn check_unbound(&self) -> Result<(), String> {
        match fs::symlink_metadata(&self.socket) {
            Ok(_) => Err("governor socket path already exists; no replacement is permitted".into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(debug(error)),
        }
    }

    pub(crate) fn check_directory(&self) -> Result<(), String> {
        self.validate()?;
        let metadata = fs::symlink_metadata(
            self.socket
                .parent()
                .ok_or("missing governor socket directory")?,
        )
        .map_err(debug)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.mode() & 0o022 != 0
            || metadata.uid() != self.supervisor.uid()
            || metadata.gid() != self.supervisor.gid()
        {
            return Err(
                "governor socket directory must be supervisor-owned and not group/other writable"
                    .into(),
            );
        }
        Ok(())
    }

    pub(super) fn secure_socket(&self) -> Result<(), String> {
        let metadata = fs::symlink_metadata(&self.socket).map_err(debug)?;
        if !metadata.file_type().is_socket()
            || metadata.uid() != self.supervisor.uid()
            || metadata.gid() != self.supervisor.gid()
        {
            return Err("governor socket has unexpected ownership or type".into());
        }
        fs::set_permissions(&self.socket, fs::Permissions::from_mode(0o660)).map_err(debug)
    }
}

pub(super) fn disjoint(left: PeerPolicy, right: PeerPolicy) -> bool {
    left.uid() != right.uid()
        || left.gid() != right.gid()
        || matches!((left.pid(), right.pid()), (Some(a), Some(b)) if a != b)
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

// Resolve existing ancestors without creating the authority store. Canonical
// comparisons also reject collisions hidden behind ancestor-directory aliases.
fn resolved(path: &Path) -> Result<PathBuf, String> {
    let mut ancestor = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_err(debug)?.join(path)
    };
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(&ancestor) {
            Ok(_) => {
                let mut canonical = fs::canonicalize(&ancestor).map_err(debug)?;
                for component in suffix.into_iter().rev() {
                    canonical.push(component);
                }
                return Ok(canonical);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                suffix.push(
                    ancestor
                        .file_name()
                        .ok_or("missing governor path ancestor")?
                        .to_os_string(),
                );
                if !ancestor.pop() {
                    return Err("missing governor path ancestor".into());
                }
            }
            Err(error) => return Err(debug(error)),
        }
    }
}

struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> {
        match value {
            Json::Object(fields) => Ok(Self(fields)),
            _ => Err("governor profile requires an object".into()),
        }
    }
    fn take(&mut self, name: &str) -> Result<Json, String> {
        self.0
            .remove(name)
            .ok_or_else(|| format!("missing governor profile field {name}"))
    }
    fn text(&mut self, name: &str) -> Result<String, String> {
        match self.take(name)? {
            Json::String(value) => Ok(value),
            _ => Err(format!("governor {name} must be text")),
        }
    }
    fn number(&mut self, name: &str) -> Result<u64, String> {
        self.take(name)?
            .as_u64()
            .ok_or_else(|| format!("governor {name} must be an unsigned integer"))
    }
    fn size(&mut self, name: &str) -> Result<usize, String> {
        usize::try_from(self.number(name)?).map_err(debug)
    }
    fn end(self) -> Result<(), String> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err("unknown governor profile field".into())
        }
    }
}
