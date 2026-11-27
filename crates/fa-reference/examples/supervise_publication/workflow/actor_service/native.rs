//! Native monitored generation feeding the ORIGINAL source-only publication gate.
//! Stdout carries a submission reference, NEVER unapproved generated text.
#[path = "native/anchors.rs"]
mod anchors;
#[path = "native/bootstrap.rs"]
mod bootstrap;
#[path = "native/continuation.rs"]
mod continuation;
#[path = "native/control.rs"]
mod control;
#[path = "native/evaluation.rs"]
mod evaluation;
#[path = "native/policy_replay.rs"]
mod policy_replay;
pub(crate) use policy_replay::NativePolicyReplay;
#[path = "native/precheck.rs"]
mod exact_precheck;
#[path = "native/finish.rs"]
mod finish;
#[path = "native/operator.rs"]
mod operator;
#[path = "native/qualification.rs"]
mod qualification;
#[path = "native/recipe.rs"]
mod recipe;
#[path = "native/recovery.rs"]
mod recovery;
#[path = "native/recovery_requirements.rs"]
mod recovery_requirements;
#[path = "native/sequence.rs"]
mod sequence;
#[cfg(test)]
#[path = "native/tests.rs"]
mod tests;

use super::*;
use super::super::FileHumanReviewer;
use control::Control;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::GenerationFinish;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::text::FileTextGenerationCommand;
use fa_reference::action::consequence::delivery::persistent::observed::stream::generated::FileTextMessageRequest;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{FileActorInbox, FileGeneratedTextActorPort};
use fa_reference::action::consequence::delivery::persistent::requests::actor::source_wire::inbox::pool::{
    FileActorPool, PoolAttachError, PoolBudget, PoolDriveReport,
};
use fa_reference::action::consequence::oversight::actor::ActorProposal;
use fa_reference::action::consequence::oversight::actor_wire::{Command, encode_command};
use std::io::Write;
use recipe::Inputs;

const USAGE: &str = "serve-create|serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE --native-text RECIPE; serve-create-checked|serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE --native-text RECIPE; native open also accepts a final --after-message PREVIOUS_REQUEST OR --finish-after-message PREVIOUS_REQUEST; native services optionally accept paired --operator-profile PATH --tower-generation NONZERO, --history-anchor INPUT on open, and --retain-history-anchor NEW_OUTPUT; single native plain or witness-checked services (including change feeds and exact snapshots) additionally accept paired --andon ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE with required --retain-history-anchor NEW_OUTPUT; evaluated open also requires --history-anchor INPUT and --evaluation-recovery REQUIREMENTS; explicit --evaluate-only reopens a recorded native Message or Finish only for independent assessment";

pub(super) fn sequence_command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    sequence::command(args, credibility)
}

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    command_with_clock(args, credibility, super::super::clock, &mut std::io::stdout().lock())
}

fn command_with_clock<F>(
    args: &[String],
    credibility: Option<&Path>,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let (args, evaluation_options) = evaluation::take_options(args)?;
    if evaluation_options.is_some() && credibility.is_some() {
        return Err("native live evaluation cannot silently replace held-out qualification".into());
    }
    let (args, history_options) = anchors::take_options(&args)?;
    let (selected_args, precheck_path) = exact_precheck::take_option(&args)?;
    let (selected_args, operator_option) = operator::take_option(&selected_args)?;
    let (args, finishing) = finish::take_option(&selected_args)?;
    let (args, after) = continuation::take_option(args)?;
    let (open, checked) = match args.first().map(String::as_str) {
        Some("serve-create") => (false, false),
        Some("serve-open") => (true, false),
        Some("serve-create-checked") => (false, true),
        Some("serve-open-checked") => (true, true),
        _ => return Err(USAGE.into()),
    };
    let option = if checked { 5 } else { 4 };
    if args.len() != option + 2 || args[option] != "--native-text" {
        return Err(USAGE.into());
    }
    let config = Config::read(Path::new(&args[1]))?;
    let actor = Profile::read(Path::new(&args[2]))?;
    let reviewer = PeerProfile::read(Path::new(&args[3]))?;
    check_profiles(&config, &actor, &reviewer)?;
    let operator = operator_option
        .map(|(path, generation)| operator::Selection::read(&path, generation))
        .transpose()?;
    if let Some(selected) = &operator {
        selected.check(&config, &actor, &reviewer)?;
    }
    let precheck = precheck_path
        .map(|path| exact_precheck::Selection::read(&path))
        .transpose()?;
    if let Some(selected) = &precheck {
        selected.check_initial_epoch(open)?;
        selected.check(&config, &actor, &reviewer, operator.as_ref())?;
    }
    let history = history_options.admit(&config, open)?;
    if let Some(selected) = &history {
        selected.check(&config, &[(&actor, precheck.as_ref())], &reviewer, operator.as_ref(), open)?;
        let mut files: Vec<_> = args[1..option].iter().map(std::path::PathBuf::from).collect();
        files.push(std::path::PathBuf::from(&args[option + 1]));
        if let Some(path) = credibility { files.push(path.to_path_buf()); }
        selected.check_files(&files)?;
    }
    let publication = if checked {
        Some(PublicationProfile::read(Path::new(&args[4]))?)
    } else {
        None
    };
    let evaluation = evaluation_options.map(|options| options.read(
        (&config, &actor, &reviewer),
        (operator.as_ref(), precheck.as_ref(), history.as_ref()),
    )).transpose()?;
    if let Some(evaluation) = &evaluation {
        evaluation.check(
            &config, &actor, &reviewer,
            (operator.as_ref(), precheck.as_ref(), history.as_ref()),
            (open, publication.as_ref(), credibility),
        )?;
        let mut paths: Vec<_> = args[1..option].iter().map(Path::new).collect();
        paths.push(Path::new(&args[option + 1]));
        evaluation.check_inputs(&paths)?;
    }
    let inputs = recipe::load_with_path_check(
        Path::new(&args[option + 1]),
        config.profile.delivery.scope.tenant,
        config.profile.delivery.limits.bytes,
        |path| {
            if let Some(history) = &history {
                history.check_files(&[path.to_path_buf()])?;
            }
            if let Some(evaluation) = &evaluation {
                evaluation.check_inputs(&[path])?;
            }
            Ok(())
        },
    )?;
    if let Some(selected) = evaluation.as_ref().filter(|selected| selected.evaluate_only()) {
        return selected.serve_history(
            config, (&actor, &reviewer), inputs, publication.as_ref(),
            history.as_ref().ok_or("historical evaluation requires independent history custody")?,
            finishing, time,
        );
    }
    if operator.is_some() || precheck.is_some() || history.is_some() {
        let mode = match (finishing, after) {
            (Some(previous), _) => finish::Intent::Finish(previous),
            (None, Some(previous)) => finish::Intent::Continue(previous),
            (None, None) => finish::Intent::Existing,
        };
        return serve_lifecycle_with_evaluation(
            config,
            (&actor, &reviewer),
            inputs,
            (open, publication.as_ref(), credibility, mode),
            Services {
                operator: operator.as_ref(), precheck: precheck.as_ref(),
                history: history.as_ref(), evaluation: evaluation.as_ref(),
            },
            time,
            output,
        );
    }
    if let Some(previous) = finishing {
        finish::serve(
            config,
            &actor,
            &reviewer,
            inputs,
            (publication.as_ref(), credibility, previous),
            time,
            output,
        )
    } else if after.is_some() {
        serve_continued(
            config,
            &actor,
            &reviewer,
            inputs,
            (open, publication.as_ref(), credibility, after),
            time,
            output,
        )
    } else if credibility.is_some() {
        serve_qualified(
            config,
            &actor,
            &reviewer,
            inputs,
            (open, publication.as_ref(), credibility),
            time,
            output,
        )
    } else if let Some(publication) = &publication {
        serve_selected(
            config,
            &actor,
            &reviewer,
            inputs,
            (open, Some(publication)),
            time,
            output,
        )
    } else if open {
        serve_mode(
            config,
            &actor,
            &reviewer,
            inputs,
            true,
            time,
            output,
        )
    } else {
        serve(
            config,
            &actor,
            &reviewer,
            inputs,
            time,
            output,
        )
    }
}

fn check_profiles(config: &Config, actor: &Profile, reviewer: &PeerProfile) -> Result<(), String> {
    actor.check_host(config)?;
    reviewer.check_host(config)?;
    if !config.profile.delivery.initial_payload.is_empty() {
        return Err("a native text stream requires an explicitly empty initial payload".into());
    }
    if actor.socket == reviewer.socket(actor.request)
        || actor.socket == super::super::control::socket_path(reviewer, actor.request)
    {
        return Err("actor, reviewer and stop endpoints must be distinct".into());
    }
    Ok(())
}

fn serve<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    inputs: Inputs,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    serve_mode(config, actor, reviewer_profile, inputs, false, time, output)
}

fn serve_mode<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    inputs: Inputs,
    open: bool,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    serve_selected(
        config,
        actor,
        reviewer_profile,
        inputs,
        (open, None),
        time,
        output,
    )
}

fn serve_selected<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    inputs: Inputs,
    deployment: (bool, Option<&PublicationProfile>),
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    serve_qualified(
        config,
        actor,
        reviewer_profile,
        inputs,
        (deployment.0, deployment.1, None),
        time,
        output,
    )
}

fn serve_qualified<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    inputs: Inputs,
    deployment: (bool, Option<&PublicationProfile>, Option<&Path>),
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    serve_continued(
        config,
        actor,
        reviewer_profile,
        inputs,
        (deployment.0, deployment.1, deployment.2, None),
        time,
        output,
    )
}

fn serve_continued<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    inputs: Inputs,
    deployment: (
        bool,
        Option<&PublicationProfile>,
        Option<&Path>,
        Option<u64>,
    ),
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let mode = deployment
        .3
        .map_or(finish::Intent::Existing, finish::Intent::Continue);
    serve_lifecycle(
        config,
        actor,
        reviewer_profile,
        inputs,
        (deployment.0, deployment.1, deployment.2, mode),
        time,
        output,
    )
}

fn serve_lifecycle<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    inputs: Inputs,
    deployment: (
        bool,
        Option<&PublicationProfile>,
        Option<&Path>,
        finish::Intent,
    ),
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    serve_lifecycle_selected(
        config,
        actor,
        reviewer_profile,
        inputs,
        deployment,
        None,
        time,
        output,
    )
}

fn serve_lifecycle_selected<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    inputs: Inputs,
    deployment: (
        bool,
        Option<&PublicationProfile>,
        Option<&Path>,
        finish::Intent,
    ),
    operator_selection: Option<&operator::Selection>,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    serve_lifecycle_with_precheck(
        config,
        actor,
        reviewer_profile,
        inputs,
        deployment,
        operator_selection,
        None,
        time,
        output,
    )
}

fn serve_lifecycle_with_precheck<F>(
    config: Config,
    actor: &Profile,
    reviewer_profile: &PeerProfile,
    inputs: Inputs,
    deployment: (
        bool,
        Option<&PublicationProfile>,
        Option<&Path>,
        finish::Intent,
    ),
    operator_selection: Option<&operator::Selection>,
    precheck_selection: Option<&exact_precheck::Selection>,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    serve_lifecycle_with_history(
        config, (actor, reviewer_profile), inputs, deployment,
        (operator_selection, precheck_selection, None), time, output,
    )
}

struct Services<'a> {
    operator: Option<&'a operator::Selection>,
    precheck: Option<&'a exact_precheck::Selection>,
    history: Option<&'a anchors::Selection>,
    evaluation: Option<&'a evaluation::Selection>,
}

fn serve_lifecycle_with_history<F>(
    config: Config,
    peers: (&Profile, &PeerProfile),
    inputs: Inputs,
    deployment: (bool, Option<&PublicationProfile>, Option<&Path>, finish::Intent),
    services: (Option<&operator::Selection>, Option<&exact_precheck::Selection>, Option<&anchors::Selection>),
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    serve_lifecycle_with_evaluation(
        config, peers, inputs, deployment,
        Services { operator: services.0, precheck: services.1, history: services.2, evaluation: None },
        time, output,
    )
}

fn serve_lifecycle_with_evaluation<F>(
    mut config: Config,
    peers: (&Profile, &PeerProfile),
    inputs: Inputs,
    deployment: (
        bool,
        Option<&PublicationProfile>,
        Option<&Path>,
        finish::Intent,
    ),
    services: Services<'_>,
    mut time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let (actor, reviewer_profile) = peers;
    let Services {
        operator: operator_selection, precheck: precheck_selection,
        history: history_selection, evaluation: evaluation_selection,
    } = services;
    let (open, publication, credibility, mode) = deployment;
    mode.check(open, actor.request)?;
    check_profiles(&config, actor, reviewer_profile)?;
    if let Some(selected) = operator_selection {
        selected.check(&config, actor, reviewer_profile)?;
    }
    if let Some(selected) = precheck_selection {
        selected.check_initial_epoch(open)?;
        selected.check(&config, actor, reviewer_profile, operator_selection)?;
    }
    if let Some(selected) = history_selection {
        selected.check(&config, &[(actor, precheck_selection)], reviewer_profile, operator_selection, open)?;
    }
    if let Some(selected) = evaluation_selection {
        if selected.evaluate_only() {
            return Err("historical native evaluation requires the evaluation-only service".into());
        }
        selected.check(
            &config, actor, reviewer_profile,
            (operator_selection, precheck_selection, history_selection),
            (open, publication, credibility),
        )?;
    }
    // Source and complete witness/joint policy belong to the same initial image.
    // Recovery checks this selection before cleaning staging or fencing keys.
    let selected = bootstrap::selection(&config, &inputs, publication)?;
    if inputs.decoder.profile().identity().tenant != config.profile.delivery.scope.tenant {
        return Err("native model tenant mismatch".into());
    }
    let runtime = actor.runtime_ms.min(config.timing.runtime_ms);
    let start = time();
    let deadline = Deadline {
        logical: plus(start, runtime)?,
        started: Instant::now(),
        wall: Duration::from_millis(runtime),
    };
    // Exclusive listener ownership precedes the durable owner. No stale path is removed.
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    // Explicit continuation and evaluated recovery request live work. Their
    // original Stop transport exists throughout replay, including Ready, and is
    // transferred into the same service without renewing its candidate quota.
    let mut recovery_control = if open
        && (matches!(mode, finish::Intent::Continue(_)) || evaluation_selection.is_some())
    {
        Some(Control::new(
            &config,
            actor.request,
            Some(reviewer_profile),
        )?)
    } else {
        None
    };
    let (host, reviewer, evaluator_role) = if let Some(evaluation) = evaluation_selection {
        let (host, reviewer, role) = if open {
            evaluation.open(
                &config, &inputs, selected,
                history_selection.ok_or("evaluated native recovery requires history custody")?,
                recovery_control.as_mut().ok_or("evaluated native recovery requires Stop custody")?,
                &deadline, &mut time,
            )?
        } else {
            evaluation.create(&config, &inputs, selected)?
        };
        (host, reviewer, Some(role))
    } else if open {
        let (host, reviewer) = bootstrap::open_with_history(
            &config,
            &inputs,
            selected,
            history_selection,
            recovery_control.as_mut(),
            &deadline,
            &mut time,
        )?;
        (host, reviewer, None)
    } else {
        let (host, reviewer) = bootstrap::prepare(&config, &inputs, selected, false)?;
        (host, reviewer, None)
    };
    // Any startup refusal after the original recovery Fence retains that
    // exact acknowledged successor while its owner is still available.
    let prepared: Result<_, String> = (|| {
        if !host.generated_text_stream_required().map_err(debug)?
            || !host.publication_guard_required()
            || host.file_source_status().map(|status| status.policy) != Some(config.source_policy)
            || host.journal_capacity().map_err(debug)?.reserve() != Some(RecoveryReserve::terminal())
            || host.publication_validation_profile().map_err(debug)?
                != publication.map(|profile| profile.limits)
        {
            return Err(
                "native source, reserve and publication contracts must match this service".into(),
            );
        }
        // New initiation is opt-in. Ordinary open still rejects a missing intent;
        // duplicate IDs must use the existing exact recovery/receipt path instead.
        let next = match mode {
            finish::Intent::Continue(after) => {
                Some(continuation::prepare(&host, actor.request, &inputs, after)?)
            }
            _ => None,
        };
        let recovered = match mode {
            finish::Intent::Finish(after) => {
                finish::select(&host, actor.request, &inputs, after, deadline.logical)?
            }
            finish::Intent::Existing if open => recovery::select(&host, actor.request, &inputs)?
                .map(|source| FileGeneratedTextActorPort::encode_message(&source).map_err(debug))
                .transpose()?,
            _ => None,
        };
        let recorded = recovered.is_some();
        if recorded && evaluation_selection.is_some() {
            return Err("recorded native requests are observation-only; evaluated recovery requires pending generation or an explicit new message/finish".into());
        }
        if recorded && precheck_selection.is_some() {
            return Err("recorded native requests are observation-only and cannot acquire a new precheck disclosure".into());
        }
        if let Some(selected) = precheck_selection {
            super::precheck::check_owner_epoch(&selected.settings, &host)?;
        }
        if recorded {
            if let Some(selected) = operator_selection {
                selected.check_recorded()?;
            }
        }
        if !recorded && credibility.is_none() {
            host.check_credibility().map_err(debug)?;
        }
        Ok((next, recovered, recorded))
    })();
    let (next, recovered, recorded) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => return match history_selection {
            Some(selected) => selected.retain(&host, Err(error)),
            None => Err(error),
        },
    };
    // This transfer itself performs no journal transition. Its failure may
    // therefore retain this exact capture, never a later stale fallback.
    let transfer = history_selection.map(|selected| selected.capture(&host)).transpose()?;
    let (port, supervisor) = match host.into_generated_text_actor_gateway() {
        Ok(gateway) => gateway,
        Err(error) => return match history_selection {
            Some(selected) => selected.finish_capture(Ok(transfer.flatten()), Err(debug(error))),
            None => Err(debug(error)),
        },
    };
    drop(transfer);
    let mut driver = FileSupervisedDriver::new(supervisor);
    let outcome: Result<(), String> = (|| {
        let operator_port = operator_selection.map(|_| port.clone());
        let inbox = FileActorInbox::new(
            actor.actor,
            ActorWire::new(port),
            actor.channels(),
            actor.connections,
        )
        .map_err(debug)?;
        let pool = FileActorPool::for_requests(vec![(actor.request, inbox)]).map_err(debug)?;
        let mut ingress = Intake {
            socket,
            pool,
            profile: actor.clone(),
            attempted: 0,
        };
        // Historical requests create no control/review listeners and never resume
        // numerical work. Their original source document is retained byte-for-byte.
        let mut control = if recorded {
            None
        } else if let Some(control) = recovery_control.take() {
            // Keep the same candidate quota, authenticated socket and deadline.
            Some(control)
        } else {
            Some(Control::new(
                &config,
                actor.request,
                Some(reviewer_profile),
            )?)
        };
        let mut operator = operator_selection
            .map(|selected| {
                selected.start(
                    actor,
                    operator_port.expect("selected native operator retains its source-only port"),
                    &driver,
                    &deadline,
                )
            })
            .transpose()?;
        let mut evaluator = match (evaluation_selection, evaluator_role) {
            (Some(selected), Some(role)) => {
                if control.as_mut().expect("evaluated live service owns Stop")
                    .checkpoint(&mut driver, &reviewer, &deadline, &mut time)?
                {
                    return Ok(());
                }
                Some(super::evaluator::Service::start(
                    &selected.peer, actor, role, &driver, &deadline,
                )?)
            }
            (None, None) => None,
            _ => return Err("native evaluator selection and original custody disagree".into()),
        };
        if let Some(control) = &mut control {
            control.install_operator(operator.take());
            control.install_evaluator(evaluator.take());
        }
        // A selected operator or evaluator retains existing Stop custody through
        // its remaining service/reply work. No selection creates another quota.
        let mut operator_stop = None;
        let mut precheck = None;
        let work = (|| {
            let proposal = if let Some(proposal) = recovered {
                // Even a supplied credibility path is not opened for receipt-only
                // recovery. The historical effect cannot be retried into new work.
                resume_existing(&mut driver, actor.request, &mut time)?;
                proposal
            } else {
                if !qualification::prepare(
                    &mut driver,
                    &config,
                    credibility,
                    (
                        control.as_mut().expect("initial stop owner"),
                        &reviewer,
                        &deadline,
                    ),
                    &mut time,
                )? {
                    return Ok(());
                }
                if let finish::Intent::Finish(after) = mode {
                    let controls = (
                        control.as_mut().expect("initial stop owner"),
                        &reviewer,
                        &deadline,
                    );
                    let Some(proposal) = finish::prepare(
                        &mut driver,
                        &mut config,
                        actor.request,
                        after,
                        controls,
                        &mut time,
                    )?
                    else {
                        return Ok(());
                    };
                    proposal
                } else {
                    let controls = (
                        control.as_mut().expect("initial stop owner"),
                        &reviewer,
                        &deadline,
                    );
                    let completed = if let Some(prepared) = next {
                        continuation::start(
                            &mut driver,
                            &mut config,
                            &inputs,
                            prepared,
                            controls,
                            &mut time,
                        )?
                    } else if open {
                        recovery::resume(&mut driver, &mut config, &inputs, controls, &mut time)?
                    } else {
                        generate(&mut driver, &mut config, &inputs, controls, &mut time)?
                    };
                    if !completed {
                        return Ok(());
                    }
                    // Only a first, unsubmitted complete generation gets a NEW proposal.
                    // An existing request above keeps its target, epoch and deadline.
                    completed_message_proposal(&driver, actor.request, &inputs, &deadline)?
                }
            };
            if let Some(selected) = precheck_selection {
                // Independent Stop remains first, including the boundary after
                // generation and before the fresh source/disclosure bootstrap.
                if control
                    .as_mut()
                    .expect("live precheck Stop owner")
                    .checkpoint(&mut driver, &reviewer, &deadline, &mut time)?
                {
                    return Ok(());
                }
                precheck = Some(super::precheck::Service::start_generated(
                    &selected.settings,
                    actor,
                    reviewer_profile,
                    &mut config,
                    &mut driver,
                    &deadline,
                    &mut time,
                    &proposal,
                )?);
            }
            let document = encode_command(&Command::Submit {
                request: actor.request,
                proposal,
            })
            .map_err(debug)?;
            output
                .write_all(&document)
                .and_then(|()| output.write_all(b"\n"))
                .and_then(|()| output.flush())
                .map_err(|error| {
                    format!("source reference output failed: {error}; no actor request sent")
                })?;
            if recorded {
                // Do not consume reply grace before an authenticated actor command.
                // Even an expired old Submit is handled by the original retry path;
                // no fresh evidence, helpers, human approval or publication occurs.
                loop {
                    deadline.check(time())?;
                    if let Some(service) = &mut operator {
                        service.checkpoint();
                    }
                    if ingress.observe_count(&mut driver)? != 0 {
                        return Ok(());
                    }
                    pause(actor.poll_ms);
                }
            }
            loop {
                deadline.check(time())?;
                if control.as_mut().expect("retained stop owner").checkpoint(
                    &mut driver,
                    &reviewer,
                    &deadline,
                    &mut time,
                )? {
                    return Ok(());
                }
                if let Some(service) = &mut precheck {
                    service.checkpoint(&driver, time());
                }
                ingress.drive(&mut driver, &mut config, &mut time)?;
                match driver
                    .supervisor()
                    .host()
                    .map_err(debug)?
                    .request_status(actor.request)
                {
                    Ok(_) => break,
                    Err(JournalError::Contract(Error::Missing)) => {}
                    Err(error) => return Err(debug(error)),
                }
                pause(actor.poll_ms);
            }
            if let Some(service) = &mut precheck {
                service.retire();
            }
            evaluator = control.as_mut().expect("original live native Stop owner").take_evaluator();
            let (stop_service, selected_operator) = control
                .take()
                .expect("original live native Stop owner")
                .into_parts();
            operator = selected_operator;
            if operator_selection.is_some() || evaluation_selection.is_some() {
                operator_stop = Some(stop_service.share());
            }
            let mut pump = |driver: &mut FileSupervisedDriver| {
                // LiveControl services the original Stop first, then observes any
                // newly acknowledged operator Fence before further workflow work.
                if let Some(service) = &mut operator {
                    service.checkpoint();
                }
                if let Some(service) = &mut evaluator {
                    service.checkpoint_driver(driver);
                }
                ingress.observe(driver)?;
                let status = driver
                    .supervisor()
                    .host()
                    .map_err(debug)?
                    .request_status(actor.request)
                    .map_err(debug)?;
                Ok(matches!(
                    status.disposition,
                    FileRequestDisposition::Admitted {
                        stage: ActionState::Cancelled
                            | ActionState::Denied
                            | ActionState::Confirmed
                            | ActionState::ConfirmedNotExecuted,
                        ..
                    }
                ))
            };
            execute_serviced(
                &mut driver,
                &reviewer,
                &mut config,
                actor.request,
                &deadline,
                ExecuteServices {
                    peers: Some(reviewer_profile),
                    publication,
                    ingress: &mut pump,
                    stop: Some(stop_service),
                },
                &mut time,
            )
        })();
        if let Some(service) = &mut precheck {
            service.retire();
        }
        if let Some(control) = &mut control {
            operator = control.take_operator();
            evaluator = control.take_evaluator();
        }
        let mut automatic_cleanup_attempted = false;
        let mut failure = work.err().map(|error| {
            // A broken receipt connection must not stop unrelated later work in the
            // recovered journal. New-work failures retain the original stop/drain.
            if recorded
                || control
                    .as_ref()
                    .is_some_and(Control::native_cleanup_started)
                || operator_stop
                    .as_ref()
                    .is_some_and(crate::workflow::control::Control::native_cleanup_started)
            {
                return error;
            }
            automatic_cleanup_attempted = true;
            match stop(&mut driver, actor.request, &mut time) {
                Ok(()) => error,
                Err(stopping) => format!("{error}; stop/drain: {stopping}"),
            }
        });
        if automatic_cleanup_attempted && (operator_selection.is_some() || evaluation_selection.is_some()) {
            // The original failure path already attempted Stop AND its drain.
            // Reply grace must not reinterpret that Stop as an unconsumed operator
            // cleanup and repeat an acknowledged or failed sweep. Only the finite
            // historical operator reply channel survives this original failure.
            drop(control.take());
            drop(operator_stop.take());
        }
        // A final Hold still needs a real independent label. Explicit evaluator
        // selection retains this SAME owner/service after workflow completion,
        // under its original cumulative quotas and both original deadlines.
        // No automatic source refresh, inference, helper launch or publication
        // is scheduled here. Explicit operator commands keep their original
        // bounded grants; they cannot make this loop restart forward execution.
        let mut actor_transport_failed = false;
        if failure.is_none() && evaluator.is_some() {
            loop {
                if evaluator.as_ref().is_none_or(super::evaluator::Service::is_retired)
                    || deadline.check(time()).is_err()
                    || driver.supervisor().host().map_or(true, |host| {
                        host.storage_failure().is_some() || host.inspect().stop.is_some()
                    })
                {
                    break;
                }
                let stopped = if let Some(control) = &mut control {
                    control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)
                } else if let Some(control) = &mut operator_stop {
                    control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)
                } else {
                    Err("original evaluated Stop custody is unavailable".into())
                };
                match stopped {
                    Ok(false) => {}
                    Ok(true) => break,
                    Err(error) => { failure.get_or_insert(error); break; }
                }
                if let Some(service) = &mut operator {
                    service.checkpoint();
                }
                let stopped = if let Some(control) = &mut control {
                    control.native_stop(&mut driver, &mut time)
                } else if let Some(control) = &mut operator_stop {
                    control.native_stop(&mut driver, &mut time)
                } else {
                    Err("original evaluated Stop custody is unavailable".into())
                };
                match stopped {
                    Ok(false) => {}
                    Ok(true) => break,
                    Err(error) => { failure.get_or_insert(error); break; }
                }
                evaluator.as_mut().expect("selected original evaluator").checkpoint_driver(&mut driver);
                if !actor_transport_failed {
                    if let Err(error) = ingress.observe(&mut driver) {
                        failure.get_or_insert(error);
                        actor_transport_failed = true;
                        ingress.pool.revoke_all();
                    }
                }
                pause(actor.poll_ms);
            }
        }
        // Terminal reply grace cannot begin new evaluation. Only an already
        // acknowledged response retains its remaining original transport budget.
        if let Some(service) = &mut evaluator {
            service.finish();
        }
        let grace = Instant::now();
        while grace.elapsed() < Duration::from_millis(actor.reply_ms) {
            if operator_selection.is_some() || evaluation_selection.is_some() {
                // Keep independent Stop ahead of operator traffic while this
                // explicitly selected live endpoint retains terminal reply custody.
                // Historical receipt-only recovery still has no Stop listener.
                let stopped = if let Some(control) = &mut control {
                    control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)
                } else if let Some(control) = &mut operator_stop {
                    control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)
                } else {
                    Ok(false)
                };
                if let Err(error) = stopped {
                    failure.get_or_insert(error);
                }
            }
            if let Some(service) = &mut operator {
                service.checkpoint();
            }
            if operator_selection.is_some() || evaluation_selection.is_some() {
                let stopped = if let Some(control) = &mut control {
                    control.native_stop(&mut driver, &mut time)
                } else if let Some(control) = &mut operator_stop {
                    control.native_stop(&mut driver, &mut time)
                } else {
                    Ok(false) // Recorded recovery has observation-only custody.
                };
                if let Err(error) = stopped {
                    failure.get_or_insert(error);
                }
            }
            if let Some(service) = &mut evaluator {
                service.checkpoint_driver(&mut driver);
            }
            if !actor_transport_failed {
                if let Err(error) = ingress.observe(&mut driver) {
                    failure.get_or_insert(error);
                    if operator_selection.is_none() && evaluation_selection.is_none() {
                        break;
                    }
                    // A broken actor channel cannot consume the independent
                    // operator's remaining original opportunity to read its reply.
                    actor_transport_failed = true;
                    ingress.pool.revoke_all();
                }
            }
            pause(actor.poll_ms);
        }
        if let Some(service) = &mut operator {
            service.retire();
        }
        if let Some(service) = &mut evaluator {
            service.retire();
        }
        ingress.pool.revoke_all();
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    })();
    // All original Stop/drain and reply work has ended. The cleanup below only
    // releases direct helper children; it cannot append a journal transition.
    let outcome = match history_selection {
        Some(selected) => selected.retain_driver(&driver, outcome),
        None => outcome,
    };
    let pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    if let Err(error) = outcome {
        return Err(format!("{error}; helper cleanup pending={pending}"));
    }
    if pending != 0 {
        return Err(format!(
            "{pending} direct helper children lack reaping confirmation"
        ));
    }
    Ok(())
}

/// Project only the fixed source reference of a completed original generation.
/// Single-release and resident services share these checks and expose no output.
fn completed_message_proposal(
    driver: &FileSupervisedDriver,
    request: u64,
    inputs: &Inputs,
    deadline: &Deadline,
) -> Result<ActorProposal, String> {
    let host = driver.supervisor().host().map_err(debug)?;
    let progress = host
        .decoder_generation_progress(inputs.generation)
        .map_err(debug)?;
    let text = host
        .decoder_text_generation(inputs.generation)
        .map_err(debug)?;
    let bytes = text.result().map_err(debug)?.bytes().map_err(debug)?;
    if bytes.is_empty() || std::str::from_utf8(bytes).is_err() {
        return Err("native result is not a nonempty complete UTF-8 message".into());
    }
    let source = FileTextMessageRequest {
        request,
        generation: inputs.generation,
        generation_revision: progress.generation_revision(),
        target: host.inspect().target,
        policy_epoch: host.inspect().control.ledger.epoch,
        deadline: deadline.logical,
    };
    FileGeneratedTextActorPort::encode_message(&source).map_err(debug)
}

/// The original intent and one-token reducer own all cursor, budget and RNG state.
fn generate<F>(
    driver: &mut FileSupervisedDriver,
    config: &mut Config,
    inputs: &Inputs,
    controls: (&mut Control, &FileHumanReviewer, &Deadline),
    time: &mut F,
) -> Result<bool, String>
where
    F: FnMut() -> ElapsedTick,
{
    let (control, reviewer, deadline) = controls;
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? {
        return Ok(false);
    }
    refresh(driver, config, time)?;
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? {
        return Ok(false);
    }
    {
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        stamp(&mut host, deadline, time)?;
        let numerical = host.decoder_inspection().map_err(debug)?.numerical;
        let command = FileTextGenerationCommand::new(
            inputs.generation,
            numerical.actor_revision,
            numerical.position,
            inputs.request.clone(),
        )
        .map_err(debug)?;
        let revision = host.revision();
        host.begin_decoder_text(revision, command).map_err(debug)?;
    }
    advance_pending(driver, config, inputs, (control, reviewer, deadline), time)
}

// Creation and explicit recovery share the SAME one-token/control/source loop.
fn advance_pending<F>(
    driver: &mut FileSupervisedDriver,
    config: &mut Config,
    inputs: &Inputs,
    controls: (&mut Control, &FileHumanReviewer, &Deadline),
    time: &mut F,
) -> Result<bool, String>
where
    F: FnMut() -> ElapsedTick,
{
    let (control, reviewer, deadline) = controls;
    loop {
        deadline.check(time())?;
        if control.checkpoint(driver, reviewer, deadline, time)? {
            return Ok(false);
        }
        let progress = driver
            .supervisor()
            .host()
            .map_err(debug)?
            .decoder_generation_progress(inputs.generation)
            .map_err(debug)?;
        if progress.is_complete() {
            if progress.finish() != Some(Ok(GenerationFinish::StopToken)) {
                return Err(format!(
                    "native generation did not complete on a monitored control token: {:?}",
                    progress.finish()
                ));
            }
            return Ok(true);
        }
        refresh(driver, config, time)?;
        deadline.check(time())?;
        if control.checkpoint(driver, reviewer, deadline, time)? {
            return Ok(false);
        }
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        stamp(&mut host, deadline, time)?;
        let revision = host.revision();
        host.advance_decoder_generation(
            revision,
            inputs.generation,
            progress.generation_revision(),
        )
        .map_err(debug)?;
        // No new-request token, source or actor I/O before the next stop checkpoint.
    }
}

// Re-sample after potentially slow source/control work. A source lease is checked
// by the ORIGINAL captured-state gate against this exact just-observed tick.
fn stamp<F>(host: &mut FileOversight, deadline: &Deadline, time: &mut F) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let now = time();
    deadline.check(now)?;
    if !host.clock_ready() || host.inspect().control.ledger.elapsed != Some(now) {
        let revision = host.revision();
        host.observe_time(revision, now).map_err(debug)?;
    }
    host.capture_file_policy_state().map_err(debug)?;
    Ok(())
}

fn refresh<F>(
    driver: &mut FileSupervisedDriver,
    config: &mut Config,
    time: &mut F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
    let revision = host.revision();
    host.refresh_file_source(revision, &mut config.source, time())
        .map_err(debug)?;
    let now = time();
    if !host.clock_ready() || host.inspect().control.ledger.elapsed != Some(now) {
        let revision = host.revision();
        host.observe_time(revision, now).map_err(debug)?;
    }
    Ok(())
}

struct Intake {
    socket: BoundSocket,
    pool: FileActorPool<FileGeneratedTextActorPort>,
    profile: Profile,
    attempted: u64,
}
impl Intake {
    fn accept(&mut self) -> Result<(), String> {
        let (_, status, _) = self
            .pool
            .statuses()
            .next()
            .ok_or("missing native actor session")?;
        if status.active.is_some() || status.revoked {
            return Ok(());
        }
        let socket = match self.socket.listener.accept() {
            Ok((socket, _)) => socket,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                return Ok(());
            }
            Err(error) => return Err(debug(error)),
        };
        if self.attempted == self.profile.candidates {
            return Err("native actor candidate limit reached".into());
        }
        self.attempted += 1;
        match self.pool.attach(self.profile.request, socket) {
            Ok(_) => Ok(()),
            Err(PoolAttachError::Refused(PeerRefusal::CredentialsRejected))
                if self.attempted < self.profile.candidates =>
            {
                Ok(())
            }
            Err(error) => Err(debug(error)),
        }
    }
    fn drive<F>(
        &mut self,
        driver: &mut FileSupervisedDriver,
        config: &mut Config,
        time: &mut F,
    ) -> Result<(), String>
    where
        F: FnMut() -> ElapsedTick,
    {
        self.accept()?;
        check_report(
            self.pool
                .drive(driver, &mut config.source, time, pool_budget())
                .map_err(debug)?,
        )
    }
    fn observe(&mut self, driver: &mut FileSupervisedDriver) -> Result<(), String> {
        self.observe_count(driver).map(|_| ())
    }
    fn observe_count(&mut self, driver: &mut FileSupervisedDriver) -> Result<usize, String> {
        self.accept()?;
        let budget = pool_budget();
        let report = self.pool.observe(driver, budget).map_err(debug)?;
        let frames = budget
            .total
            .frames
            .checked_sub(report.remaining.frames)
            .ok_or("actor observation exceeded its issued frame allowance")?;
        check_report(report)?;
        Ok(frames)
    }
}
fn pool_budget() -> PoolBudget {
    PoolBudget {
        total: DriveBudget {
            frames: 1,
            ..DriveBudget::default()
        },
        ..PoolBudget::default()
    }
}
fn check_report(report: PoolDriveReport) -> Result<(), String> {
    for visit in report.visits {
        visit.result.map_err(debug)?;
    }
    Ok(())
}
