//! Independently selected exact checks for one resident native sequence.
//! Admission freezes the whole finite selection before any recipe/model read.
//! Each original grant lives only until its own source-only actor admission;
//! queries, reconnects and handoff cannot replenish its declared allowance.

use super::super::{exact_precheck, operator};
use super::{
    ActionSpec, ActorProposal, Command, Config, ElapsedTick, Entry, EntryServices, Error,
    FileSupervisedDriver, JournalError, PeerProfile, Profile, Write, debug, encode_command, pause,
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
    if let Some(selected) = first {
        selected.check_initial_epoch(after.is_some())?;
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
    previous: Option<u64>,
    selected: (&mut ActorProposal, &mut ActionSpec),
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
        governed,
        ..
    } = services;
    let (proposal, action) = selected;
    let mut refresh = PolicyRefresh {
        enabled: governed,
        used: false,
        policy_generation: driver
            .supervisor()
            .host()
            .map_err(debug)?
            .current_policy()
            .map_err(debug)?
            .generation(),
    };
    let mut service = None;
    let result = (|| {
        if let Some(selected) = &entry.precheck {
            if control.checkpoint(driver, reviewer, deadline, &mut *time)? {
                return Ok(false);
            }
            // An independently selected disclosure is never rebound or renewed
            // after promotion. The actor may use a newly emitted source intent,
            // but any old exact-precheck grant has ended at its declared epoch.
            refresh.update(driver, entry, previous, deadline, proposal, action)?;
            if !refresh.used {
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
        }
        emit(entry.actor.request, proposal, output)?;
        loop {
            deadline.check(time())?;
            if control.checkpoint(driver, reviewer, deadline, &mut *time)? {
                return Ok(false);
            }
            if refresh.update(driver, entry, previous, deadline, proposal, action)? {
                if let Some(mut stale) = service.take() {
                    stale.retire();
                }
                emit(entry.actor.request, proposal, output)?;
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
            if refresh.update(driver, entry, previous, deadline, proposal, action)? {
                if let Some(mut stale) = service.take() {
                    stale.retire();
                }
                emit(entry.actor.request, proposal, output)?;
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

fn emit(request: u64, proposal: &ActorProposal, output: &mut impl Write) -> Result<(), String> {
    let document = encode_command(&Command::Submit {
        request,
        proposal: proposal.clone(),
    })
    .map_err(debug)?;
    output
        .write_all(&document)
        .and_then(|()| output.write_all(b"\n"))
        .and_then(|()| output.flush())
        .map_err(|error| format!("source reference output failed: {error}; no actor request sent"))
}

/// Exactly one fresh projection may follow the original governor's single
/// promotion. A source reference is input, never a rewrite of an admitted
/// request: any recorded disposition keeps its original frozen intent.
struct PolicyRefresh {
    enabled: bool,
    used: bool,
    policy_generation: u64,
}

impl PolicyRefresh {
    fn update(
        &mut self,
        driver: &FileSupervisedDriver,
        entry: &Entry,
        previous: Option<u64>,
        deadline: &super::Deadline,
        proposal: &mut ActorProposal,
        action: &mut ActionSpec,
    ) -> Result<bool, String> {
        if !self.enabled {
            return Ok(false);
        }
        let host = driver.supervisor().host().map_err(debug)?;
        match host.request_status(entry.actor.request) {
            Ok(_) => return Ok(false),
            Err(JournalError::Contract(Error::Missing)) => {}
            Err(error) => return Err(debug(error)),
        }
        let epoch = host.inspect().control.ledger.epoch;
        if epoch == proposal.expected_policy_epoch {
            return Ok(false);
        }
        if self.used
            || proposal.expected_policy_epoch.checked_add(1) != Some(epoch)
            || host.current_policy().map_err(debug)?.generation() == self.policy_generation
        {
            return Err(
                "native source reference cannot refresh a non-promotion authority change".into(),
            );
        }
        let fresh = match entry.release {
            super::Release::Message => super::completed_message_proposal(
                driver,
                entry.actor.request,
                &entry.inputs,
                deadline,
            )?,
            super::Release::Finish => {
                let intent = host
                    .prepare_decoder_text_finish(
                        previous.ok_or("native Finish refresh needs its original predecessor")?,
                        entry.actor.request,
                        proposal.deadline,
                    )
                    .map_err(debug)?;
                super::FileGeneratedTextActorPort::encode_finish(
                    entry.actor.request,
                    intent.target,
                    intent.expected_policy_epoch,
                    intent.deadline,
                )
                .map_err(debug)?
            }
        };
        let fresh_action = super::selected_action(driver, entry, &fresh)?;
        let mut same_intent = fresh.clone();
        same_intent.expected_policy_epoch = proposal.expected_policy_epoch;
        let mut same_action = fresh_action.clone();
        same_action.policy_epoch = action.policy_epoch;
        if same_intent != *proposal || same_action != *action {
            return Err("native policy refresh changed the selected source, target, deadline or full action".into());
        }
        // No clock, source capture, inference, permit or journal mutation occurs.
        // The independently authenticated actor must submit this NEW envelope.
        *proposal = fresh;
        *action = fresh_action;
        self.used = true;
        Ok(true)
    }
}
