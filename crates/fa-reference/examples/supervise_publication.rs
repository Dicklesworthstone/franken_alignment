//! Runnable Unix reference publication and independent human-review commands.
//! All policy, input, helper, approval, publication and reconciliation decisions
//! come from the existing fa-reference owners, not this command's orchestration.
#![forbid(unsafe_code)]
#[cfg(unix)]
#[path = "supervise_publication/config.rs"]
mod config;
#[cfg(unix)]
#[path = "supervise_publication/console.rs"]
mod console;
#[cfg(unix)]
#[path = "supervise_publication/diagnosis.rs"]
mod diagnosis;
#[cfg(unix)]
#[path = "supervise_publication/guarded_resume.rs"]
mod guarded_resume;
#[cfg(unix)]
#[path = "supervise_publication/peers.rs"]
mod peers;
#[cfg(unix)]
#[path = "supervise_publication/proposal.rs"]
mod proposal;
#[cfg(unix)]
#[path = "supervise_publication/publication.rs"]
mod publication;
#[cfg(all(test, unix))]
#[path = "supervise_publication/publication_tests.rs"]
mod publication_tests;
#[cfg(unix)]
#[path = "supervise_publication/qualification.rs"]
mod qualification;
#[cfg(unix)]
#[path = "supervise_publication/recover_stop.rs"]
mod recover_stop;
#[cfg(all(test, unix))]
#[path = "supervise_publication/test_directory.rs"]
mod test_directory;
#[cfg(all(test, unix))]
#[path = "supervise_publication/tests.rs"]
mod tests;
#[cfg(unix)]
#[path = "supervise_publication/workflow.rs"]
mod workflow;

#[cfg(unix)]
fn main() {
    if let Err(error) = command(std::env::args().skip(1).collect()) {
        eprintln!("supervise_publication: {error}");
        std::process::exit(1);
    }
}
#[cfg(not(unix))]
fn main() {
    eprintln!("supervise_publication requires the Unix reference profile");
    std::process::exit(1);
}

#[cfg(unix)]
fn command(mut args: Vec<String>) -> Result<(), String> {
    use config::{Config, debug, read_regular};
    use fa_reference::action::MAX_PAYLOAD_BYTES;
    use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
    use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
    use fa_reference::action::consequence::oversight::actor_wire::MAX_FRAME_BYTES;
    use peers::PeerProfile;
    use std::io::Write;
    use std::path::Path;
    let usage = "usage: supervise_publication create CONFIG SUBMIT_JSON [--retain-history-anchor OUTPUT] [--reviewer-profile REVIEWER_JSON]\n       supervise_publication submit CONFIG SUBMIT_JSON [--guarded RECOVERY_PROFILE] [--reviewer-profile REVIEWER_JSON] [--credibility-activation EVIDENCE_FILE]\n       supervise_publication resume CONFIG SUBMIT_JSON [--guarded RECOVERY_PROFILE]\n       supervise_publication create-checked CONFIG SUBMIT_JSON WITNESS_PROFILE [--retain-history-anchor OUTPUT] [--reviewer-profile REVIEWER_JSON]\n       supervise_publication submit-checked CONFIG SUBMIT_JSON WITNESS_PROFILE [--reviewer-profile REVIEWER_JSON] [--credibility-activation EVIDENCE_FILE]\n       supervise_publication resume-checked CONFIG SUBMIT_JSON WITNESS_PROFILE\n       supervise_publication review CONFIG REQUEST_ID\n       supervise_publication review-peer REVIEWER_JSON REQUEST_ID\n       supervise_publication stop-peer REVIEWER_JSON REQUEST_ID\n       supervise_publication stop-fleet FLEET_JSON\n       supervise_publication proposal CONFIG REQUEST_ID PAYLOAD_FILE TTL_MS\n       supervise_publication proposal-next CONFIG REQUEST_ID PAYLOAD_FILE TTL_MS [--credibility-activation EVIDENCE_FILE]\n       supervise_publication recover-stop CONFIG OPERATION [--guarded RECOVERY_PROFILE]\n       supervise_publication stop-admission CONFIG OPERATION [--retain-history-anchor NEW_FILE]\n       supervise_publication drain-stopped CONFIG OPERATION [--retain-history-anchor NEW_FILE]\n       supervise_publication inspect CONFIG\n       supervise_publication explain-proposal CONFIG REQUEST_ID RECOVERY_PROFILE [--payload REPLACEMENT_FILE]\n       supervise_publication serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE\n       supervise_publication serve-open CONFIG ACTOR_PROFILE REVIEWER_PROFILE [--credibility-activation EVIDENCE_FILE]\n       supervise_publication serve-create-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE\n       supervise_publication serve-open-checked CONFIG ACTOR_PROFILE REVIEWER_PROFILE WITNESS_PROFILE [--credibility-activation EVIDENCE_FILE]\n       supervise_publication actor-submit ACTOR_PROFILE SUBMIT_JSON\n       supervise_publication create-generated CONFIG GENERATION_RECIPE REVIEWER_PROFILE\n       supervise_publication create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE [--continue-generation]\n       supervise_publication serve-learned-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE [--continue-generation | --after-message PREVIOUS_REQUEST] [--topology-profile PATH] [--history-anchor INPUT] [--retain-history-anchor OUTPUT] [--credibility-activation EVIDENCE_FILE]\n       supervise_publication create-learned-generated CONFIG LEARNED_RECIPE --resume\n       supervise_publication create-learned-generated CONFIG LEARNED_RECIPE (--recover-stop | --stop-admission | --drain-stopped) OPERATION [--topology-profile PATH] [--history-anchor INPUT] [--retain-history-anchor OUTPUT]";
    // The independent fleet roster owns its complete grammar and never reads Config.
    if args.first().is_some_and(|mode| mode == "stop-fleet") {
        return peers::fleet::command(&args, &mut std::io::stdout().lock());
    }
    // Native accounting owns its complete grammar before any option is removed.
    // Unsupported guard/activation combinations cannot fall back to legacy work.
    if args.iter().skip(3).any(|value| value == "--andon") {
        return workflow::continuation::andon::command(&args, &mut std::io::stdout().lock());
    }
    // Terminal phases own a closed option set before any shared option removal.
    if args
        .first()
        .is_some_and(|mode| matches!(mode.as_str(), "stop-admission" | "drain-stopped"))
    {
        return recover_stop::admission::command(&args, &mut std::io::stdout().lock());
    }
    // Read-only diagnosis has a closed option set and owns one elapsed budget
    // beginning before CONFIG is read. No live-service profile can be substituted.
    if args.first().is_some_and(|mode| mode == "explain-proposal") {
        return diagnosis::command(&args, &mut std::io::stdout().lock());
    }
    // Initial custody is an explicit create-only intent with a closed flag set.
    // Do not consume/ignore recovery or credibility flags before validating it.
    if args
        .first()
        .is_some_and(|mode| matches!(mode.as_str(), "create" | "create-checked"))
        && args
            .iter()
            .skip(3)
            .any(|value| value == "--retain-history-anchor")
    {
        return workflow::continuation::creation::command(&args, &mut std::io::stdout().lock());
    }
    let credibility = qualification::take_option(&mut args)?;
    if args.first().is_some_and(|mode| {
        matches!(
            mode.as_str(),
            "serve-create"
                | "serve-open"
                | "serve-create-checked"
                | "serve-open-checked"
                | "actor-submit"
                | "create-generated"
                | "create-learned-generated"
                | "serve-learned-generated"
        )
    }) {
        #[cfg(target_os = "linux")]
        {
            return workflow::actor_service::command(&args, credibility.as_deref());
        }
        #[cfg(not(target_os = "linux"))]
        {
            return Err(
                "live actor service requires Linux peer credentials; no unchecked fallback".into(),
            );
        }
    }
    if args.first().is_some_and(|mode| mode == "submit")
        && args.iter().skip(3).any(|value| value == "--guarded")
    {
        return workflow::continuation::guarded::command(
            &args,
            credibility.as_deref(),
            &mut std::io::stdout().lock(),
        );
    }
    let peer_profile = if (args.len() == 5
        && matches!(args[0].as_str(), "create" | "submit")
        && args[3] == "--reviewer-profile")
        || (args.len() == 6
            && matches!(args[0].as_str(), "create-checked" | "submit-checked")
            && args[4] == "--reviewer-profile")
    {
        let path = args.pop().ok_or(usage)?;
        args.pop();
        Some(PeerProfile::read(Path::new(&path))?)
    } else {
        None
    };
    let mode = args.first().map(String::as_str).ok_or(usage)?;
    let expected = match mode {
        "resume" | "recover-stop" if args.len() == 5 && args[3] == "--guarded" => 5,
        "create" | "submit" | "resume" | "review" | "review-peer" | "stop-peer"
        | "recover-stop" => 3,
        "create-checked" | "submit-checked" | "resume-checked" => 4,
        "proposal" | "proposal-next" => 5,
        "inspect" => 2,
        _ => return Err(usage.into()),
    };
    if args.len() != expected {
        return Err(usage.into());
    }
    // This role-specific path must not parse the private supervisor file merely
    // to review or stop. A checked profile failure never retries a legacy channel.
    if matches!(mode, "review-peer" | "stop-peer") {
        let profile = PeerProfile::read(Path::new(&args[1]))?;
        let request = args[2].parse::<u64>().map_err(debug)?;
        if request == 0 {
            return Err("request must be nonzero".into());
        }
        if mode == "stop-peer" {
            return peers::stop::request(&profile, request, &mut std::io::stdout().lock());
        }
        return console::review_peer(&profile, request);
    }
    // Parse the operator file and original actor document before creating a store
    // or starting a program. No profile flag silently selects an easier fallback.
    let config = Config::read(Path::new(&args[1]))?;
    if mode == "resume" && args.len() == 5 {
        let document = read_regular(Path::new(&args[2]), MAX_FRAME_BYTES)?;
        let profile = recover_stop::guarded::ResumeProfile::read(Path::new(&args[4]))?;
        return guarded_resume::run(
            &config,
            &profile,
            &document,
            workflow::clock,
            &mut std::io::stdout().lock(),
        );
    }
    match mode {
        "create" | "submit" | "resume" | "create-checked" | "submit-checked" | "resume-checked" => {
            let document = read_regular(Path::new(&args[2]), MAX_FRAME_BYTES)?;
            let result = if matches!(mode, "create-checked" | "submit-checked" | "resume-checked") {
                let publication = publication::PublicationProfile::read(Path::new(&args[3]))?;
                if mode == "submit-checked" {
                    match credibility.as_deref() {
                        Some(path) => workflow::continuation::submit_with_credibility(
                            config,
                            &document,
                            peer_profile.as_ref(),
                            Some(&publication),
                            Some(path),
                            workflow::clock,
                        )?,
                        None => workflow::continuation::submit_existing(
                            config,
                            &document,
                            peer_profile.as_ref(),
                            Some(&publication),
                            workflow::clock,
                        )?,
                    }
                } else {
                    workflow::run_with_publication(
                        config,
                        &document,
                        mode == "resume-checked",
                        peer_profile.as_ref(),
                        Some(&publication),
                        workflow::clock,
                    )?
                }
            } else if mode == "submit" {
                match credibility.as_deref() {
                    Some(path) => workflow::continuation::submit_with_credibility(
                        config,
                        &document,
                        peer_profile.as_ref(),
                        None,
                        Some(path),
                        workflow::clock,
                    )?,
                    None => workflow::continuation::submit_existing(
                        config,
                        &document,
                        peer_profile.as_ref(),
                        None,
                        workflow::clock,
                    )?,
                }
            } else {
                match &peer_profile {
                    Some(profile) => workflow::run_with_peers(
                        config,
                        &document,
                        false,
                        Some(profile),
                        workflow::clock,
                    )?,
                    None => workflow::run(config, &document, mode == "resume", workflow::clock)?,
                }
            };
            let executed = matches!(
                &result.response.result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::Executed,
                    ..
                })
            );
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(&result.response.encode()).map_err(debug)?;
            stdout.write_all(b"\n").map_err(debug)?;
            stdout.flush().map_err(debug)?;
            // An actor-visible result comes only from its original gateway.
            // A failed output cannot unwind committed work or trigger a resend.
            if let Some(error) = result.failure {
                return Err(error);
            }
            if result.cleanup_pending != 0 {
                return Err(format!(
                    "{} direct helper children lack reaping confirmation",
                    result.cleanup_pending
                ));
            }
            if !executed {
                return Err(
                    "the original actor result does not confirm execution; see the JSON response"
                        .into(),
                );
            }
            Ok(())
        }
        "recover-stop" => {
            let operation = args[2].parse::<u64>().map_err(debug)?;
            if args.len() == 5 {
                let profile = recover_stop::guarded::Profile::read(Path::new(&args[4]))?;
                recover_stop::guarded::run(
                    &config,
                    &profile,
                    operation,
                    workflow::clock,
                    &mut std::io::stdout().lock(),
                )
            } else {
                recover_stop::run(
                    &config,
                    operation,
                    workflow::clock,
                    &mut std::io::stdout().lock(),
                )
            }
        }
        "review" => {
            let request = args[2].parse::<u64>().map_err(debug)?;
            if request == 0 {
                return Err("request must be nonzero".into());
            }
            console::review(&config, request)
        }
        "proposal" | "proposal-next" => {
            let request = args[2].parse::<u64>().map_err(debug)?;
            let payload = read_regular(Path::new(&args[3]), MAX_PAYLOAD_BYTES)?;
            let ttl = args[4].parse::<u64>().map_err(debug)?;
            let basis = if mode == "proposal-next" {
                proposal::Basis::Existing
            } else {
                proposal::Basis::Bootstrap
            };
            let bytes = match credibility.as_deref() {
                Some(path) => {
                    let qualification = qualification::read(path)?;
                    proposal::document_qualified(
                        &config,
                        request,
                        payload,
                        ttl,
                        workflow::clock(),
                        &qualification,
                    )?
                }
                None => {
                    proposal::document(&config, request, payload, ttl, workflow::clock(), basis)?
                }
            };
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(&bytes).map_err(debug)?;
            stdout.write_all(b"\n").map_err(debug)?;
            stdout.flush().map_err(debug)
        }
        "inspect" => {
            // This is privileged, historical inspection, not an actor API or a
            // fresh source observation. Preserve raw payload bytes through hex.
            let state =
                FileOversight::read_publication(&config.store, &config.profile).map_err(debug)?;
            let payload: String = state.payload.iter().map(|b| format!("{b:02x}")).collect();
            println!(
                "{{\"revision\":\"{}\",\"executions\":\"{}\",\"version\":\"{}\",\"payload_hex\":\"{}\",\"charged\":\"{}\",\"reserved\":\"{}\"}}",
                state.revision,
                state.executions,
                state.target.expected_version,
                payload,
                state.control.ledger.charged,
                state.control.ledger.reserved
            );
            Ok(())
        }
        _ => unreachable!("validated subcommand"),
    }
}
