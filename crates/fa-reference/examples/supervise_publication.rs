//! Runnable Unix reference publication and independent human-review commands.
//! All policy, input, helper, approval, publication and reconciliation decisions
//! come from the existing fa-reference owners, not this command's orchestration.
#![forbid(unsafe_code)]
#[cfg(unix)]
#[path = "supervise_publication/config.rs"] mod config;
#[cfg(unix)]
#[path = "supervise_publication/workflow.rs"] mod workflow;
#[cfg(unix)]
#[path = "supervise_publication/console.rs"] mod console;
#[cfg(all(test, unix))]
#[path = "supervise_publication/tests.rs"] mod tests;

#[cfg(unix)]
fn main() {
    if let Err(error) = command(std::env::args().skip(1).collect()) {
        eprintln!("supervise_publication: {error}"); std::process::exit(1);
    }
}
#[cfg(not(unix))]
fn main() { eprintln!("supervise_publication requires the Unix reference profile"); std::process::exit(1); }

#[cfg(unix)]
fn command(args: Vec<String>) -> Result<(), String> {
    use config::{Config, debug, read_regular};
    use fa_reference::action::{ElapsedTick, MAX_PAYLOAD_BYTES};
    use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
    use fa_reference::action::consequence::oversight::actor::{ActorProposal, ActorOutcome, Knowledge};
    use fa_reference::action::consequence::oversight::actor_wire::{Command, MAX_FRAME_BYTES, encode_command};
    use std::io::Write;
    use std::path::Path;
    let usage = "usage: supervise_publication create|resume CONFIG SUBMIT_JSON\n       supervise_publication review CONFIG REQUEST_ID\n       supervise_publication proposal CONFIG REQUEST_ID PAYLOAD_FILE TTL_MS\n       supervise_publication inspect CONFIG";
    let mode = args.first().map(String::as_str).ok_or(usage)?;
    let expected = match mode { "create" | "resume" | "review" => 3, "proposal" => 5, "inspect" => 2, _ => return Err(usage.into()) };
    if args.len() != expected { return Err(usage.into()); }
    // Parse the operator file and original actor document before creating a store
    // or starting a program. No profile flag silently selects an easier fallback.
    let config = Config::read(Path::new(&args[1]))?;
    match mode {
        "create" | "resume" => {
            let document = read_regular(Path::new(&args[2]), MAX_FRAME_BYTES)?;
            let result = workflow::run(config, &document, mode == "resume", workflow::clock)?;
            let executed = matches!(&result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }));
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(&result.response.encode()).map_err(debug)?;
            stdout.write_all(b"\n").map_err(debug)?; stdout.flush().map_err(debug)?;
            // An actor-visible result comes only from its original gateway.
            // A failed output cannot unwind committed work or trigger a resend.
            if let Some(error) = result.failure { return Err(error); }
            if result.cleanup_pending != 0 { return Err(format!("{} direct helper children lack reaping confirmation", result.cleanup_pending)); }
            if !executed { return Err("the original actor result does not confirm execution; see the JSON response".into()); }
            Ok(())
        }
        "review" => {
            let request = args[2].parse::<u64>().map_err(debug)?;
            if request == 0 { return Err("request must be nonzero".into()); }
            console::review(&config, request)
        }
        "proposal" => {
            let request = args[2].parse::<u64>().map_err(debug)?;
            let payload = read_regular(Path::new(&args[3]), MAX_PAYLOAD_BYTES)?;
            let ttl = args[4].parse::<u64>().map_err(debug)?;
            if ttl == 0 || ttl > config.timing.runtime_ms { return Err("TTL must fit the configured one-shot runtime".into()); }
            let deadline = workflow::clock().0.checked_add(ttl).ok_or("deadline overflow")?;
            let proposal = ActorProposal { target: config.profile.delivery.target,
                units: u64::try_from(payload.len()).map_err(debug)?.max(1), payload,
                deadline: ElapsedTick(deadline), expected_policy_epoch: 0 };
            let bytes = encode_command(&Command::Submit { request, proposal }).map_err(debug)?;
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(&bytes).map_err(debug)?; stdout.write_all(b"\n").map_err(debug)?; stdout.flush().map_err(debug)
        }
        "inspect" => {
            // This is privileged, historical inspection, not an actor API or a
            // fresh source observation. Preserve raw payload bytes through hex.
            let state = FileOversight::read_publication(&config.store, &config.profile).map_err(debug)?;
            let payload: String = state.payload.iter().map(|b| format!("{b:02x}")).collect();
            println!("{{\"revision\":\"{}\",\"executions\":\"{}\",\"version\":\"{}\",\"payload_hex\":\"{}\",\"charged\":\"{}\",\"reserved\":\"{}\"}}",
                state.revision, state.executions, state.target.expected_version, payload,
                state.control.ledger.charged, state.control.ledger.reserved);
            Ok(())
        }
        _ => unreachable!("validated subcommand"),
    }
}
