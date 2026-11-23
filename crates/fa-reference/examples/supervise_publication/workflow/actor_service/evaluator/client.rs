//! One explicit authenticated assessment; a lost response never retries it.
use super::{
    Settings,
    assessment::MAX_ASSESSMENT_BYTES,
    protocol::{self, Request},
};
use crate::config::{debug, read_regular};
use std::{
    io::Write,
    net::Shutdown,
    os::unix::net::UnixStream,
    path::Path,
    time::{Duration, Instant},
};

#[allow(dead_code)]
#[path = "../../../../native_andon/peer/transport.rs"]
mod transport;

pub(crate) fn command(args: &[String], output: &mut impl Write) -> Result<(), String> {
    if args.len() != 3
        || args[0] != "evaluator-assess"
        || args[1..]
            .iter()
            .any(|arg| arg.is_empty() || arg.starts_with("--"))
    {
        return Err("usage: evaluator-assess EVALUATOR_PROFILE ASSESSMENT_JSON".into());
    }
    send(Path::new(&args[1]), Path::new(&args[2]), output)
}

fn send(profile: &Path, assessment: &Path, output: &mut impl Write) -> Result<(), String> {
    let started = Instant::now();
    let settings = Settings::read(profile)?;
    let wall = Duration::from_millis(settings.runtime_ms);
    let mut checks = 0_usize;
    let mut checkpoint = || {
        checks = checks
            .checked_add(1)
            .ok_or("evaluator client work overflow")?;
        if started.elapsed() >= wall || checks > super::MAX_IO_CALLS {
            Err("one original evaluator client lifetime/work allowance exhausted; completion may be unknown".into())
        } else {
            Ok(())
        }
    };
    checkpoint()?;
    let request = Request::prepare(&settings, &read_regular(assessment, MAX_ASSESSMENT_BYTES)?)?;
    checkpoint()?;
    let mut socket = UnixStream::connect(&settings.socket).map_err(debug)?;
    checkpoint()?;
    settings
        .supervisor
        .policy()?
        .verify(&socket)
        .map_err(|_| "unverified supervisor; no assessment bytes disclosed")?;
    checkpoint()?;
    let mut wait = || std::thread::sleep(Duration::from_millis(settings.poll_ms));
    transport::write(&mut socket, &request.wire, &mut checkpoint, &mut wait)?;
    checkpoint()?;
    socket.shutdown(Shutdown::Write).map_err(debug)?;
    let receipt = transport::read(
        &mut socket,
        protocol::RESPONSE_BYTES,
        &mut checkpoint,
        &mut wait,
    )?;
    let outcome = protocol::validate(&request, &receipt)?;
    checkpoint()?;
    let reported = output
        .write_all(&receipt)
        .and_then(|()| output.write_all(b"\n"))
        .and_then(|()| output.flush())
        .map_err(debug);
    let late = checkpoint();
    let native = if outcome.completed() {
        Ok(())
    } else {
        Err("assessment refused or completion unconfirmed; inspect the original owner before an explicit retry".into())
    };
    let errors: Vec<_> = [native, reported, late]
        .into_iter()
        .filter_map(Result::err)
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{}; acknowledged work was not rolled back or automatically retried",
            errors.join("; ")
        ))
    }
}
