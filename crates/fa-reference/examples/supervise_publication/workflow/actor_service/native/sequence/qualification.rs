//! Explicit requalification belongs to the recovered original owner, never to
//! an actor or a later resident handoff. Input paths are admitted before live
//! custody; the existing bounded capsule reader runs under native Stop/deadline.
use super::super::{Config, FileOversight, PeerProfile, Profile, exact_precheck, operator};
use std::path::Path;

pub(super) fn check_mode(after: Option<u64>, source: Option<&Path>) -> Result<(), String> {
    if source.is_some() && after.is_none_or(|value| value == 0) {
        return Err(
            "native sequence credibility activation requires explicit --after-message recovery"
                .into(),
        );
    }
    Ok(())
}

pub(super) fn check_paths(
    config: &Config,
    entries: &[(&Profile, Option<&exact_precheck::Selection>)],
    peers: &PeerProfile,
    selected_operator: Option<&operator::Selection>,
    source: Option<&Path>,
) -> Result<(), String> {
    let Some(source) = source else {
        return Ok(());
    };
    let source = operator::resolve(source)?;
    if source.starts_with(operator::resolve(&config.store)?)
        || source == operator::resolve(config.source.path())?
    {
        return Err(
            "native credibility input must be independent of source and authority-store custody"
                .into(),
        );
    }
    for (actor, precheck) in entries {
        for endpoint in [
            actor.socket.clone(),
            peers.socket(actor.request),
            crate::workflow::control::socket_path(peers, actor.request),
        ] {
            if source == operator::resolve(&endpoint)? {
                return Err(
                    "native credibility input conflicts with a scheduled transport endpoint".into(),
                );
            }
        }
        if let Some(selected) = precheck
            && source == operator::resolve(selected.settings.socket_path())?
        {
            return Err(
                "native credibility input conflicts with a scheduled precheck endpoint".into(),
            );
        }
    }
    if let Some(selected) = selected_operator
        && source == operator::resolve(&selected.settings.socket)?
    {
        return Err(
            "native credibility input conflicts with the original operator endpoint".into(),
        );
    }
    Ok(())
}

/// The existing original activation requires exactly one epoch advance. Check
/// the independently selected first disclosure before reading the capsule, then
/// the caller rechecks the actual epoch after successful native activation.
pub(super) fn check_epoch(
    selected: Option<&exact_precheck::Selection>,
    host: &FileOversight,
    activating: bool,
) -> Result<(), String> {
    let Some(selected) = selected else {
        return Ok(());
    };
    if !activating {
        return super::super::super::precheck::check_owner_epoch(&selected.settings, host);
    }
    let epoch = host
        .inspect()
        .control
        .ledger
        .epoch
        .checked_add(1)
        .ok_or("native credibility activation policy epoch exhausted")?;
    if selected.settings.expected_policy_epoch() != epoch {
        return Err("precheck expected policy epoch must name the original owner's selected activation successor".into());
    }
    Ok(())
}
