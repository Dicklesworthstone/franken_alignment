//! Explicit live inspection and separately budgeted receipt reconciliation.
//! The selected port cannot admit actor proposals, read a policy source or
//! supply publication keys.
use super::super::{Profile, operator};
use super::{
    Config, Deadline, FileLearnedTextStreamActorPort, FileSupervisedDriver, Loaded, PeerProfile,
    fields, precheck, topology,
};
use std::path::{Path, PathBuf};

pub(super) type Service = operator::Service;

const USAGE: &str = "learned operator service requires paired --operator-profile PATH --tower-generation NONZERO in a live learned actor or resident sequence command";

/// Own the complete live grammar before any other option can erase an
/// incompatible receipt, audit, Stop, reset or maintenance selection.
pub(super) fn take_option(
    args: &[String],
) -> Result<(Vec<String>, Option<(PathBuf, u64)>), String> {
    if !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--operator-profile" | "--tower-generation"))
    {
        return Ok((args.to_vec(), None));
    }
    let single = args.first().map(String::as_str) == Some("create-learned-generated");
    let resident = args.first().map(String::as_str) == Some("serve-learned-generated");
    if args.len() < 8
        || (!single && !resident)
        || args[1..4]
            .iter()
            .any(|arg| arg.is_empty() || arg.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    let mut output = args[..4].to_vec();
    let mut profile = None;
    let mut generation = None;
    let mut actor = false;
    let mut continuation = false;
    let mut seen = std::collections::BTreeSet::new();
    let mut at = 4;
    while at < args.len() {
        let flag = args[at].as_str();
        if !seen.insert(flag) {
            return Err(USAGE.into());
        }
        if matches!(
            flag,
            "--continue-generation" | "--after-message" | "--finish-after-message"
        ) && std::mem::replace(&mut continuation, true)
        {
            return Err(USAGE.into());
        }
        if flag == "--continue-generation" {
            output.push(args[at].clone());
            at += 1;
            continue;
        }
        let common = matches!(
            flag,
            "--operator-profile"
                | "--tower-generation"
                | "--after-message"
                | "--history-anchor"
                | "--retain-history-anchor"
                | "--topology-profile"
                | "--campaign-profile"
                | "--checkpoint-before-generation"
        );
        let actor_option = single
            && matches!(
                flag,
                "--actor-profile" | "--precheck-profile" | "--finish-after-message"
            );
        if !common && !actor_option {
            return Err(USAGE.into());
        }
        let value = args.get(at + 1).ok_or(USAGE)?;
        if value.is_empty() || value.starts_with("--") {
            return Err(USAGE.into());
        }
        match flag {
            "--operator-profile" => profile = Some(PathBuf::from(value)),
            "--tower-generation" => {
                if !value.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(USAGE.into());
                }
                let selected = value.parse::<u64>().map_err(|_| USAGE.to_owned())?;
                if selected == 0 {
                    return Err(USAGE.into());
                }
                generation = Some(selected);
            }
            _ => {
                actor |= flag == "--actor-profile";
                output.extend_from_slice(&args[at..at + 2]);
            }
        }
        at += 2;
    }
    if single && !actor {
        return Err(USAGE.into());
    }
    Ok((
        output,
        Some((profile.ok_or(USAGE)?, generation.ok_or(USAGE)?)),
    ))
}

pub(super) struct Selection {
    pub settings: operator::Settings,
    pub generation: u64,
}

impl Selection {
    pub(super) fn read(path: &Path, generation: u64) -> Result<Self, String> {
        if generation == 0 {
            return Err(USAGE.into());
        }
        let settings = operator::Settings::read(path)?;
        settings.check_learned_operator()?;
        Ok(Self { settings, generation })
    }

    /// Admission inspects only selected names and host identities. It precedes
    /// the first clock, policy read, listener claim and durable owner startup.
    pub(super) fn check(
        &self,
        config: &Config,
        loaded: &Loaded,
        actor: &Profile,
        peers: &PeerProfile,
        precheck: Option<&precheck::Settings>,
    ) -> Result<(), String> {
        if self.generation == 0 {
            return Err(USAGE.into());
        }
        self.settings.check_learned_operator()?;
        let extra = precheck
            .map(|settings| settings.socket_path())
            .into_iter()
            .collect::<Vec<_>>();
        self.settings.check_host(config, actor, peers, &extra)?;
        let socket = fields::resolve_root(&self.settings.socket)?;
        if socket.starts_with(fields::resolve_root(&config.store)?) {
            return Err(
                "learned operator socket must be outside the original authority store".into(),
            );
        }
        let review_socket = peers.socket(actor.request);
        let stop_socket = crate::workflow::control::socket_path(peers, actor.request);
        let mut reserved = vec![
            actor.socket.as_path(),
            review_socket.as_path(),
            stop_socket.as_path(),
            config.source.path(),
        ];
        reserved.extend(extra);
        if let Some(path) = loaded.anchor_output.as_deref() {
            reserved.push(path);
        }
        let topology_output = match loaded.topology.as_ref() {
            None => None,
            Some(topology::Selection::Create(selected)) => selected.acknowledged_output(),
            Some(topology::Selection::Recover { selection, .. }) => selection.acknowledged_output(),
        };
        if let Some(path) = topology_output {
            reserved.push(path);
        }
        for path in reserved {
            if socket == fields::resolve_root(path)? {
                return Err("learned operator socket conflicts with original transport, source or retained output custody".into());
            }
        }
        Ok(())
    }

    pub(super) fn start(
        &self,
        actor: &Profile,
        port: FileLearnedTextStreamActorPort,
        driver: &FileSupervisedDriver,
        deadline: &Deadline,
    ) -> Result<Service, String> {
        Service::start_learned(
            &self.settings,
            actor,
            port,
            driver,
            self.generation,
            deadline,
        )
    }
}

/// Broken or exhausted peers retire only their original sidecar. A separately
/// granted Fence may stop the original owner; every driving caller must observe
/// that native transition before another source, numerical or helper operation.
pub(super) fn checkpoint(service: &mut Option<Service>) {
    if let Some(service) = service {
        service.checkpoint();
    }
}
