//! Explicit end-of-stream effects through the ORIGINAL native two-key workflow.
//! A generation stop, exhausted output budget or process exit is never closure.
use super::*;

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    if args.len() != 6 || args[0] != "create-generated" || args[4] != "--finish"
        || args[3].starts_with("--") || credibility.is_some() {
        return Err(USAGE.into());
    }
    let request = request_id(&args[5])?;
    let config = Config::read(Path::new(&args[1]))?;
    let peers = PeerProfile::read(Path::new(&args[3]))?;
    peers.check_host(&config)?;
    // This is the PREVIOUS message's original recipe, not another prompt or a
    // new generation. Only the separate finish request and its lifetime are new.
    let loaded = recipe::load(Path::new(&args[2]), &config)?;
    let result = run(config, loaded, &peers, request, clock)?;
    emit(result, &mut std::io::stdout().lock())
}

fn request_id(raw: &str) -> Result<u64, String> {
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(USAGE.into());
    }
    let id = raw.parse::<u64>().map_err(|_| USAGE.to_owned())?;
    if id == 0 { return Err(USAGE.into()); }
    Ok(id)
}

// Bind the independently loaded recipe to the original source-linked message.
// The native finish builder separately proves it is the latest CONFIRMED cut.
fn check_source(host: &FileOversight, loaded: &Loaded) -> Result<(), String> {
    let message = host.decoder_text_message_snapshot(loaded.request).map_err(debug)?;
    if message.source.generation != loaded.generation
        || host.decoder_text_generation(loaded.generation).map_err(debug)?.command().request() != &loaded.text {
        return Err("finish predecessor does not match the original native recipe".into());
    }
    Ok(())
}

pub(super) fn run<F>(mut config: Config, loaded: Loaded, peers: &PeerProfile,
    request: u64, mut time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    peers.check_host(&config)?;
    loaded.check_host(&config)?;
    if request == 0 || request == loaded.request {
        return Err("finish needs a distinct nonzero effect request".into());
    }
    let start = time();
    let lifetime = loaded.ttl_ms.min(config.timing.runtime_ms);
    let deadline = Deadline { logical: plus(start, lifetime)?, started: Instant::now(),
        wall: Duration::from_millis(lifetime) };
    deadline.check(start)?;
    let mut control = Control::new(&config, request, Some(peers))?;
    let (host, reviewer) = recovery::open(&config, &loaded)?;
    check_source(&host, &loaded)?;
    host.check_credibility().map_err(debug)?;
    let prepared = host.prepare_decoder_text_finish(loaded.request, request, deadline.logical).map_err(debug)?;
    let (port, supervisor) = host.into_generated_text_actor_gateway().map_err(debug)?;
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let work = (|| {
        if control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)? { return Ok(()); }
        refresh(&mut driver, &mut config, &deadline, &mut time)?;
        {
            let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
            let n = host.decoder_inspection().map_err(debug)?.numerical;
            let revision = host.revision();
            // Restore readiness, NOT computation. No generation is begun,
            // advanced, cancelled, reset or extended to manufacture a finish.
            host.resume_decoder(revision, n.actor_revision, n.position).map_err(debug)?;
            let current = host.prepare_decoder_text_finish(loaded.request, request, deadline.logical).map_err(debug)?;
            if current != prepared { return Err("native finish predecessor changed".into()); }
        }
        let proposal = FileGeneratedTextActorPort::encode_finish(request, prepared.target,
            prepared.expected_policy_epoch, prepared.deadline).map_err(debug)?;
        let document = encode_command(&Command::Submit { request, proposal }).map_err(debug)?;
        if control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)? { return Ok(()); }
        let submitted = driver.exchange_generated_actor_from_file(&mut wire, &document,
            &mut config.source, &mut time).map_err(debug)?;
        if submitted.response.result.is_err() { return Err("original finish gateway refused submission".into()); }
        // All helper, human, dispatch, endpoint and settlement logic stays in the
        // existing consumer. The original stream builder supplies the full frame.
        let mut no_ingress = |_: &mut FileSupervisedDriver| Ok(false);
        execute_serviced(&mut driver, &reviewer, &mut config, request, &deadline,
            ExecuteServices { peers: Some(peers), publication: None, ingress: &mut no_ingress,
                stop: Some(control) }, &mut time)
    })();
    let failure = work.err().map(|error| match stop(&mut driver, request, &mut time) {
        Ok(()) => error, Err(stopping) => format!("{error}; stop/drain: {stopping}"),
    });
    let response = wire.exchange(&encode_command(&Command::Poll { request }).map_err(debug)?);
    let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    Ok(RunResult { response, failure, cleanup_pending })
}

#[cfg(test)]
#[path = "finish/tests.rs"]
mod tests;
