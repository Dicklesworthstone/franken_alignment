//! Independently selected exact checks for one resident native sequence.
//! Admission freezes the whole finite selection before any recipe/model read.
//! Each original grant lives only until its own source-only actor admission;
//! queries, reconnects and handoff cannot replenish its declared allowance.

use super::super::{exact_precheck, operator};
use super::{
    ActorProposal, Command, Config, ElapsedTick, Entry, EntryServices, Error, FileSupervisedDriver,
    JournalError, PeerProfile, Profile, Write, debug, encode_command, pause,
};
use crate::workflow::actor_service::precheck as shared;
use std::collections::BTreeSet;

/// A new original owner starts at policy epoch zero. Refuse an impossible first
/// disclosure before loading numerical assets or consuming exclusive store
/// creation. Recovery and future rows still bind to their actual owner epoch;
/// neither can infer that epoch from the selected predecessor or row number.
pub(super) fn check_initial_epoch(
    first: Option<&exact_precheck::Selection>,
    after: Option<u64>,
) -> Result<(), String> {
    if after.is_none()
        && first.is_some_and(|selected| selected.settings.expected_policy_epoch() != 0)
    {
        return Err("first native sequence precheck must target initial policy epoch zero".into());
    }
    Ok(())
}

/// Check all future endpoints, including absent names, before any live owner,
/// clock, model input or listener is acquired. Version one adds no disclosure
/// path observations. An operator selection supplies its original endpoint;
/// it never transfers operator purpose to a precheck.
pub(super) fn check(
    config: &Config,
    selections: &[(&Profile, Option<&exact_precheck::Selection>)],
    peers: &PeerProfile,
    selected_operator: Option<&operator::Selection>,
) -> Result<(), String> {
    if selections.iter().all(|(_, selected)| selected.is_none()) {
        return Ok(());
    }
    shared::Settings::check_schedule(
        selections
            .iter()
            .filter_map(|(_, selected)| selected.map(|value| &value.settings)),
    )?;
    let store = operator::resolve(&config.store)?;
    let mut reserved = BTreeSet::from([operator::resolve(config.source.path())?]);
    for (actor, _) in selections {
        reserved.insert(operator::resolve(&actor.socket)?);
        reserved.insert(operator::resolve(&peers.socket(actor.request))?);
        reserved.insert(operator::resolve(&crate::workflow::control::socket_path(
            peers,
            actor.request,
        ))?);
    }
    if let Some(selected) = selected_operator {
        reserved.insert(operator::resolve(&selected.settings.socket)?);
    }
    let mut sockets = BTreeSet::new();
    for (actor, selected) in selections {
        let Some(selected) = selected else {
            continue;
        };
        selected.check(config, actor, peers, selected_operator)?;
        let socket = operator::resolve(selected.settings.socket_path())?;
        if socket.starts_with(&store) || reserved.contains(&socket) || !sockets.insert(socket) {
            return Err("resident native precheck socket conflicts with original source, store, operator or scheduled transport custody".into());
        }
    }
    Ok(())
}

/// Expose only the completed original source intent, then await independent
/// actor submission. This scope retires the finite disclosure on every exit,
/// including failed output, Stop, source failure and malformed actor traffic,
/// before helper review or the next release can begin.
pub(super) fn admit<F>(
    driver: &mut FileSupervisedDriver,
    config: &mut Config,
    entry: &Entry,
    proposal: &ActorProposal,
    services: EntryServices<'_>,
    time: &mut F,
    output: &mut impl Write,
) -> Result<bool, String>
where
    F: FnMut() -> ElapsedTick,
{
    let EntryServices {
        control,
        reviewer,
        peers,
        deadline,
        ingress,
        ..
    } = services;
    let mut service = None;
    let result = (|| {
        if let Some(selected) = &entry.precheck {
            if control.checkpoint(driver, reviewer, deadline, &mut *time)? {
                return Ok(false);
            }
            service = Some(shared::Service::start_generated(
                &selected.settings,
                &entry.actor,
                peers,
                config,
                driver,
                deadline,
                time,
                proposal,
            )?);
        }
        let document = encode_command(&Command::Submit {
            request: entry.actor.request,
            proposal: proposal.clone(),
        })
        .map_err(debug)?;
        output
            .write_all(&document)
            .and_then(|()| output.write_all(b"\n"))
            .and_then(|()| output.flush())
            .map_err(|error| {
                format!("source reference output failed: {error}; no actor request sent")
            })?;
        loop {
            deadline.check(time())?;
            if control.checkpoint(driver, reviewer, deadline, &mut *time)? {
                return Ok(false);
            }
            if let Some(service) = &mut service {
                service.checkpoint(driver, time());
                // A complete or partial query never outranks independent Stop
                // or extends the original deadline before actor mutation.
                deadline.check(time())?;
                if control.checkpoint(driver, reviewer, deadline, &mut *time)? {
                    return Ok(false);
                }
            }
            ingress.drive(driver, config, &mut *time)?;
            match driver
                .supervisor()
                .host()
                .map_err(debug)?
                .request_status(entry.actor.request)
            {
                Ok(_) => return Ok(true),
                Err(JournalError::Contract(Error::Missing)) => {}
                Err(error) => return Err(debug(error)),
            }
            pause(entry.actor.poll_ms);
        }
    })();
    if let Some(service) = &mut service {
        service.retire();
    }
    result
}
