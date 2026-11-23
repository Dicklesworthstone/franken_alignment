//! Keep the independently selected evaluator distinct from every live effect role.
use super::{
    super::{Config, PeerProfile, Profile, operator},
    Settings,
};
use crate::config::debug;
use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;
use fa_reference::round::review::andon::LedgerIdentity;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

impl Settings {
    pub(crate) fn socket_path(&self) -> &Path {
        &self.socket
    }

    pub(crate) fn check_host(
        &self,
        config: &Config,
        actor: &Profile,
        reviewer: &PeerProfile,
        operator: Option<&operator::Settings>,
        requests: &[u64],
        identity: LedgerIdentity,
    ) -> Result<(), String> {
        actor.check_host(config)?;
        reviewer.check_host(config)?;
        self.check_audience(config.profile.delivery.scope, identity)?;
        self.check_actor(actor)?;
        let evaluator = self.evaluator.policy()?;
        if self.supervisor.policy()? != reviewer.supervisor_policy()?
            || !disjoint(evaluator, reviewer.reviewer_policy()?)
            || operator.is_some_and(|selected| {
                selected.scope != self.scope
                    || self.supervisor.policy().ok() != Some(selected.supervisor)
                    || !disjoint(evaluator, selected.operator)
            })
        {
            return Err("evaluator requires the same supervisor/audience and distinct actor, reviewer/Stop and operator credential sets".into());
        }
        let socket = resolved(&self.socket)?;
        if socket.starts_with(resolved(&config.store)?) {
            return Err("evaluator socket must remain outside the authority store".into());
        }
        let mut endpoints = vec![actor.socket.clone()];
        if let Some(selected) = operator {
            endpoints.push(selected.socket.clone());
        }
        for &request in requests {
            endpoints.push(reviewer.socket(request));
            endpoints.push(super::super::super::control::socket_path(reviewer, request));
        }
        for endpoint in endpoints {
            if socket == resolved(&endpoint)? {
                return Err(
                    "evaluator socket conflicts with another scheduled service endpoint".into(),
                );
            }
        }
        self.check_unbound()
    }

    pub(super) fn check_actor(&self, actor: &Profile) -> Result<(), String> {
        if self.scope != actor.scope
            || self.supervisor.policy()? != actor.supervisor
            || !disjoint(self.evaluator.policy()?, actor.actor)
            || self.socket.as_os_str().as_encoded_bytes().len() > 100
            || self.socket.file_name().is_none()
        {
            return Err("evaluator audience/server mismatch, overlapping actor credentials or invalid socket length".into());
        }
        self.check_directory()
    }

    pub(crate) fn check_custody_paths(&self, paths: &[&Path]) -> Result<(), String> {
        let socket = resolved(&self.socket)?;
        for path in paths {
            if socket == resolved(path)? {
                return Err("evaluator socket conflicts with configuration, source or retained history custody".into());
            }
        }
        Ok(())
    }

    pub(super) fn check_unbound(&self) -> Result<(), String> {
        match fs::symlink_metadata(&self.socket) {
            Ok(_) => {
                Err("evaluator socket path already exists; no replacement is permitted".into())
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(debug(error)),
        }
    }
}

fn disjoint(left: PeerPolicy, right: PeerPolicy) -> bool {
    left.uid() != right.uid()
        || left.gid() != right.gid()
        || matches!((left.pid(), right.pid()), (Some(a), Some(b)) if a != b)
}

// Existing-ancestor resolution also catches aliases of a not-yet-created archive.
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
                for part in suffix.into_iter().rev() {
                    canonical.push(part);
                }
                return Ok(canonical);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                suffix.push(
                    ancestor
                        .file_name()
                        .ok_or("missing evaluator path ancestor")?
                        .to_os_string(),
                );
                if !ancestor.pop() {
                    return Err("missing evaluator path ancestor".into());
                }
            }
            Err(error) => return Err(debug(error)),
        }
    }
}
