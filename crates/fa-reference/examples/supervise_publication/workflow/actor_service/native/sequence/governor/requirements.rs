//! Reserve shared complete native recovery expectations against live services.
use super::super::super::{Config, PeerProfile, Profile, exact_precheck, operator};
use std::path::Path;

pub(in super::super) use super::super::super::recovery_requirements::Selection;

/// Reserve the frozen expectation's canonical name against every future
/// listener before recipes or numerical assets are loaded.
pub(in super::super) fn check_path(
    path: &Path,
    config: &Config,
    entries: &[(&Profile, Option<&exact_precheck::Selection>)],
    peers: &PeerProfile,
    selected_operator: Option<&operator::Selection>,
    governor: Option<&super::Settings>,
) -> Result<(), String> {
    let selected = operator::resolve(path)?;
    if selected.starts_with(operator::resolve(&config.store)?)
        || selected == operator::resolve(config.source.path())?
    {
        return Err("native recovery requirements need independent custody outside the authority store and evidence source".into());
    }
    let mut endpoints = Vec::new();
    for (actor, precheck) in entries {
        endpoints.extend([
            actor.socket.clone(),
            peers.socket(actor.request),
            crate::workflow::control::socket_path(peers, actor.request),
        ]);
        if let Some(precheck) = precheck {
            endpoints.push(precheck.settings.socket_path().to_path_buf());
        }
    }
    if let Some(operator) = selected_operator {
        endpoints.push(operator.settings.socket.clone());
    }
    if let Some(governor) = governor {
        endpoints.push(governor.socket.clone());
    }
    for endpoint in endpoints {
        if selected == operator::resolve(&endpoint)? {
            return Err(
                "native recovery requirements conflict with a selected service endpoint".into(),
            );
        }
    }
    Ok(())
}
