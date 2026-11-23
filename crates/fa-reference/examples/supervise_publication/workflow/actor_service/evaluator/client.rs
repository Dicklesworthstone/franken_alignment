//! One explicit authenticated case read or assessment; no automatic retries.
use super::{
    Settings,
    assessment::MAX_ASSESSMENT_BYTES,
    protocol::{
        self, Request,
        case::{self, CaseRequest},
    },
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
        || !matches!(args[0].as_str(), "evaluator-assess" | "evaluator-case")
        || args[1..]
            .iter()
            .any(|arg| arg.is_empty() || arg.starts_with("--"))
    {
        return Err("usage: evaluator-assess EVALUATOR_PROFILE ASSESSMENT_JSON | evaluator-case EVALUATOR_PROFILE ROUND".into());
    }
    let input = if args[0] == "evaluator-case" {
        let round = &args[2];
        if !round.bytes().all(|byte| byte.is_ascii_digit()) || round.starts_with('0') {
            return Err("evaluation round must be a nonzero canonical decimal integer".into());
        }
        Input::Case(round.parse().map_err(debug)?)
    } else {
        Input::Assessment(Path::new(&args[2]))
    };
    send(Path::new(&args[1]), input, output)
}

enum Input<'a> {
    Assessment(&'a Path),
    Case(u64),
}

enum Outbound {
    Assessment(Request),
    Case(CaseRequest),
}

impl Outbound {
    fn wire(&self) -> &[u8] {
        match self {
            Self::Assessment(request) => &request.wire,
            Self::Case(request) => &request.wire,
        }
    }

    fn validate(&self, wire: &[u8]) -> Result<bool, String> {
        match self {
            Self::Assessment(request) => {
                protocol::validate(request, wire).map(|outcome| outcome.completed())
            }
            Self::Case(request) => case::validate(request, wire),
        }
    }

    fn incomplete(&self) -> &'static str {
        match self {
            Self::Assessment(_) => "assessment refused or completion unconfirmed; inspect the original owner before an explicit retry",
            Self::Case(_) => "original evaluation case is unknown; no case identity or labeling authority was disclosed",
        }
    }
}

fn send(profile: &Path, input: Input<'_>, output: &mut impl Write) -> Result<(), String> {
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
    let request = match input {
        Input::Assessment(path) => Outbound::Assessment(Request::prepare(
            &settings,
            &read_regular(path, MAX_ASSESSMENT_BYTES)?,
        )?),
        Input::Case(round) => Outbound::Case(CaseRequest::prepare(&settings, round)?),
    };
    checkpoint()?;
    let mut socket = UnixStream::connect(&settings.socket).map_err(debug)?;
    checkpoint()?;
    settings
        .supervisor
        .policy()?
        .verify(&socket)
        .map_err(|_| "unverified supervisor; no evaluator request bytes disclosed")?;
    checkpoint()?;
    let mut wait = || std::thread::sleep(Duration::from_millis(settings.poll_ms));
    transport::write(&mut socket, request.wire(), &mut checkpoint, &mut wait)?;
    checkpoint()?;
    socket.shutdown(Shutdown::Write).map_err(debug)?;
    let receipt = transport::read(
        &mut socket,
        protocol::RESPONSE_BYTES,
        &mut checkpoint,
        &mut wait,
    )?;
    let completed = request.validate(&receipt)?;
    checkpoint()?;
    let reported = output
        .write_all(&receipt)
        .and_then(|()| output.write_all(b"\n"))
        .and_then(|()| output.flush())
        .map_err(debug);
    let late = checkpoint();
    let native = if completed {
        Ok(())
    } else {
        Err(request.incomplete().into())
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
