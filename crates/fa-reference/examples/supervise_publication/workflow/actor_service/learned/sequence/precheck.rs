//! Admit every independently selected resident disclosure before startup.
//! Only names and public profiles are checked here. The original Intake checks
//! the actual owner epoch after qualification and before numerical work, then
//! bootstraps one request-bound grant during its original admission phase.
use super::{Config, Entry, PeerProfile};
use super::super::{fields, precheck, topology};
use std::collections::BTreeSet;

pub(super) fn check(config: &Config, entries: &[Entry], peers: &PeerProfile)
    -> Result<(), String>
{
    // Preserve the original version-one path without adding path observations.
    if entries.iter().all(|entry| entry.precheck.is_none()) { return Ok(()); }
    let store = fields::resolve_root(&config.store)?;
    let mut reserved = BTreeSet::from([fields::resolve_root(config.source.path())?]);
    for entry in entries {
        reserved.insert(fields::resolve_root(&entry.actor.socket)?);
        reserved.insert(fields::resolve_root(&peers.socket(entry.loaded.request))?);
        reserved.insert(fields::resolve_root(
            &crate::workflow::control::socket_path(peers, entry.loaded.request))?);
        if let Some(path) = entry.loaded.anchor_output.as_deref() {
            reserved.insert(fields::resolve_root(path)?);
        }
        let output = match entry.loaded.topology.as_ref() {
            None => None,
            Some(topology::Selection::Create(selected)) => selected.acknowledged_output(),
            Some(topology::Selection::Recover { selection, .. }) => selection.acknowledged_output(),
        };
        if let Some(path) = output { reserved.insert(fields::resolve_root(path)?); }
    }
    let mut disclosed = BTreeSet::new();
    for entry in entries {
        let Some(settings) = entry.precheck.as_ref() else { continue; };
        precheck::check(settings, config, Some(&entry.actor), peers)?;
        let socket = fields::resolve_root(settings.socket_path())?;
        if socket.starts_with(&store) || reserved.contains(&socket)
            || !disclosed.insert(socket) {
            return Err("resident learned precheck socket conflicts with original source, store, transport or retained output custody".into());
        }
    }
    Ok(())
}
