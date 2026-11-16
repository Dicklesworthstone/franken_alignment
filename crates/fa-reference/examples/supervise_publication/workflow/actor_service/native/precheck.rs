//! Exact policy disclosure for one original native Message or Finish intent.
//! The profile selects a finite sidecar; actors still submit only source IDs.

use super::{Config, PeerProfile, Profile, operator};
use crate::workflow::actor_service::precheck as shared;
use std::path::{Path, PathBuf};

const USAGE: &str = "native text precheck requires one --precheck-profile PATH alongside --native-text RECIPE; create/open and checked forms may also select paired --operator-profile PATH --tower-generation NONZERO and one explicit open continuation or Finish";

/// Own the entire selected native grammar before another parser can consume an
/// incompatible option. Without a selection, preserve the existing grammar.
pub(super) fn take_option(args: &[String]) -> Result<(Vec<String>, Option<PathBuf>), String> {
    if !args.iter().any(|arg| arg == "--precheck-profile") {
        return Ok((args.to_vec(), None));
    }
    let (open, checked) = match args.first().map(String::as_str) {
        Some("serve-create") => (false, false),
        Some("serve-open") => (true, false),
        Some("serve-create-checked") => (false, true),
        Some("serve-open-checked") => (true, true),
        _ => return Err(USAGE.into()),
    };
    let positional = if checked { 5 } else { 4 };
    if args.len() < positional + 4
        || (args.len() - positional) % 2 != 0
        || args[1..positional]
            .iter()
            .any(|value| value.is_empty() || value.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    let mut native = None;
    let mut precheck = None;
    let mut operator = None;
    let mut generation = None;
    let mut continuation = None;
    for pair in args[positional..].chunks_exact(2) {
        if pair[1].is_empty() || pair[1].starts_with("--") {
            return Err(USAGE.into());
        }
        match pair[0].as_str() {
            "--native-text" if native.is_none() => native = Some(pair[1].clone()),
            "--precheck-profile" if precheck.is_none() => {
                precheck = Some(PathBuf::from(&pair[1]));
            }
            "--operator-profile" if operator.is_none() => operator = Some(pair[1].clone()),
            "--tower-generation" if generation.is_none() => generation = Some(pair[1].clone()),
            "--after-message" | "--finish-after-message" if open && continuation.is_none() => {
                continuation = Some(pair.to_vec());
            }
            _ => return Err(USAGE.into()),
        }
    }
    if operator.is_some() != generation.is_some() {
        return Err(USAGE.into());
    }
    let mut remaining = args[..positional].to_vec();
    remaining.extend(["--native-text".to_owned(), native.ok_or(USAGE)?]);
    if let (Some(operator), Some(generation)) = (operator, generation) {
        remaining.extend([
            "--operator-profile".to_owned(),
            operator,
            "--tower-generation".to_owned(),
            generation,
        ]);
    }
    if let Some(continuation) = continuation {
        remaining.extend(continuation);
    }
    Ok((remaining, Some(precheck.ok_or(USAGE)?)))
}

pub(super) struct Selection {
    pub(super) settings: shared::Settings,
}

impl Selection {
    pub(super) fn new(settings: shared::Settings) -> Result<Self, String> {
        shared::check_generated(&settings)?;
        Ok(Self { settings })
    }

    pub(super) fn read(path: &Path) -> Result<Self, String> {
        Self::new(shared::Settings::read(path)?)
    }

    /// A fresh original owner has epoch zero before any operator is installed.
    /// Reject an impossible selection before consuming its exclusive store
    /// creation. An opener still checks the actual recovered owner afterward.
    pub(super) fn check_initial_epoch(&self, open: bool) -> Result<(), String> {
        if !open && self.settings.expected_policy_epoch() != 0 {
            return Err("native precheck creation requires initial policy epoch zero".into());
        }
        Ok(())
    }

    /// Validate all endpoint aliases before a clock, model load, generation or
    /// durable startup. Canonical parents preserve symlink alias detection even
    /// for listeners and stores that do not exist yet.
    pub(super) fn check(
        &self,
        config: &Config,
        actor: &Profile,
        reviewer: &PeerProfile,
        selected_operator: Option<&operator::Selection>,
    ) -> Result<(), String> {
        shared::check_generated(&self.settings)?;
        self.settings.check_host(config, actor, reviewer)?;
        let socket = operator::resolve(self.settings.socket_path())?;
        if socket.starts_with(operator::resolve(&config.store)?) {
            return Err(
                "native precheck socket must be outside the original authority store".into(),
            );
        }
        let review = reviewer.socket(actor.request);
        let stop = crate::workflow::control::socket_path(reviewer, actor.request);
        for path in [
            actor.socket.as_path(),
            review.as_path(),
            stop.as_path(),
            config.source.path(),
        ] {
            if socket == operator::resolve(path)? {
                return Err(
                    "native precheck socket conflicts with original transport or source custody"
                        .into(),
                );
            }
        }
        if let Some(selected) = selected_operator {
            if socket == operator::resolve(&selected.settings.socket)? {
                return Err("native precheck and operator sockets must be distinct".into());
            }
        }
        Ok(())
    }
}
