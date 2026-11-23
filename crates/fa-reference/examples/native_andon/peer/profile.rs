//! Closed evaluator peer profile shared by offline and same-owner live intake.
//! Parsing a profile grants no evaluator or publication authority.
use crate::config::{debug, read_regular};
use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;
use fa_reference::strict_json::{self, Json, Limits};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};

const PROFILE_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Identity {
    pub(crate) uid: u32,
    pub(crate) gid: u32,
    pub(crate) pid: Option<u32>,
}
impl Identity {
    pub(crate) fn policy(self) -> Result<PeerPolicy, String> {
        PeerPolicy::new(self.uid, self.gid, self.pid).map_err(debug)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EvaluatorPeerProfile {
    pub(crate) socket: PathBuf,
    pub(crate) scope: fa_reference::action::Scope,
    pub(crate) deployment: u64,
    pub(crate) evaluator_id: u64,
    pub(crate) supervisor: Identity,
    pub(crate) evaluator: Identity,
    pub(crate) candidates: u32,
    pub(crate) runtime_ms: u64,
    pub(crate) poll_ms: u64,
}
impl EvaluatorPeerProfile {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, PROFILE_BYTES)?)
    }
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let json = strict_json::parse(
            bytes,
            Limits {
                max_bytes: PROFILE_BYTES,
                max_depth: 5,
                max_items: 64,
                max_string_bytes: 4096,
            },
        )
        .map_err(debug)?;
        let root = object(
            &json,
            &[
                "schema_version",
                "socket",
                "scope",
                "deployment",
                "evaluator",
                "supervisor",
                "peer",
                "candidate_limit",
                "runtime_ms",
                "poll_ms",
            ],
        )?;
        if root["schema_version"].as_str() != Some("fa.native-evaluator-peer/1") {
            return Err("unsupported native evaluator peer profile".into());
        }
        let scope_fields = object(
            &root["scope"],
            &["tenant", "principal", "run", "branch", "authority"],
        )?;
        let scope = fa_reference::action::Scope {
            tenant: number(scope_fields, "tenant")?,
            principal: number(scope_fields, "principal")?,
            run: number(scope_fields, "run")?,
            branch: number(scope_fields, "branch")?,
            authority: number(scope_fields, "authority")?,
            purpose: fa_reference::action::Purpose::Effect,
        };
        if [
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
        ]
        .contains(&0)
        {
            return Err("evaluator peer scope identities must be nonzero".into());
        }
        let socket = PathBuf::from(
            root["socket"]
                .as_str()
                .ok_or("evaluator socket must be text")?,
        );
        if !socket.is_absolute()
            || socket
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
            || socket.as_os_str().as_encoded_bytes().contains(&0)
        {
            return Err("evaluator socket must be an absolute normalized path without NUL".into());
        }
        let deployment = number(root, "deployment")?;
        let evaluator_id = number(root, "evaluator")?;
        let supervisor = identity(&root["supervisor"])?;
        let evaluator = identity(&root["peer"])?;
        supervisor.policy()?;
        evaluator.policy()?;
        let candidates = u32::try_from(number(root, "candidate_limit")?).map_err(debug)?;
        let runtime_ms = number(root, "runtime_ms")?;
        let poll_ms = number(root, "poll_ms")?;
        if deployment == 0
            || evaluator_id == 0
            || candidates == 0
            || candidates > 1024
            || runtime_ms == 0
            || runtime_ms > 3_600_000
            || poll_ms == 0
            || poll_ms > 1000
            || poll_ms > runtime_ms
        {
            return Err("invalid finite evaluator peer limits".into());
        }
        Ok(Self {
            socket,
            scope,
            deployment,
            evaluator_id,
            supervisor,
            evaluator,
            candidates,
            runtime_ms,
            poll_ms,
        })
    }
    pub(crate) fn check_audience(
        &self,
        scope: fa_reference::action::Scope,
        id: fa_reference::round::review::andon::LedgerIdentity,
    ) -> Result<(), String> {
        if self.scope != scope
            || self.deployment != id.deployment
            || self.evaluator_id != id.evaluator
        {
            return Err(
                "evaluator peer profile does not match the native accounting audience".into(),
            );
        }
        self.check_directory()
    }
    pub(crate) fn check_directory(&self) -> Result<(), String> {
        let parent = self
            .socket
            .parent()
            .ok_or("evaluator socket requires a parent directory")?;
        let meta = fs::symlink_metadata(parent).map_err(debug)?;
        if !meta.is_dir()
            || meta.file_type().is_symlink()
            || meta.mode() & 0o022 != 0
            || meta.uid() != self.supervisor.uid
            || meta.gid() != self.supervisor.gid
        {
            return Err(
                "evaluator socket directory must be supervisor-owned and not group/other writable"
                    .into(),
            );
        }
        Ok(())
    }
}

fn object<'a>(value: &'a Json, fields: &[&str]) -> Result<&'a BTreeMap<String, Json>, String> {
    let Json::Object(object) = value else {
        return Err("evaluator peer profile expects an object".into());
    };
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err("evaluator peer profile has missing or unsupported fields".into());
    }
    Ok(object)
}
fn number(object: &BTreeMap<String, Json>, field: &str) -> Result<u64, String> {
    object[field]
        .as_u64()
        .ok_or_else(|| format!("{field} must be an unsigned integer"))
}
fn identity(value: &Json) -> Result<Identity, String> {
    let fields = object(value, &["uid", "gid", "pid"])?;
    let uid = u32::try_from(number(fields, "uid")?).map_err(debug)?;
    let gid = u32::try_from(number(fields, "gid")?).map_err(debug)?;
    let pid = match &fields["pid"] {
        Json::Null => None,
        value => Some(
            u32::try_from(value.as_u64().ok_or("pid must be null or unsigned")?).map_err(debug)?,
        ),
    };
    Ok(Identity { uid, gid, pid })
}
