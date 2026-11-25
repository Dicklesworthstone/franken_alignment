//! One explicit authenticated case read or assessment; no automatic retries.
use super::{
    Settings,
    assessment::MAX_ASSESSMENT_BYTES,
    protocol::{
        self, Request,
        case::{self, CaseRequest},
        cases::{self, CasesRequest},
        pending::{self, PendingRequest},
    },
};
use crate::config::{debug, read_regular};
use fa_reference::action::consequence::delivery::persistent::observed::credibility::{
    FileEvaluationCasesBasis, MAX_EVALUATION_CASE_PAGE,
};
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
    const USAGE: &str = "usage: evaluator-assess EVALUATOR_PROFILE ASSESSMENT_JSON | evaluator-case EVALUATOR_PROFILE ROUND | evaluator-case EVALUATOR_PROFILE --list LIMIT [--after ROUND --at-journal REV --at-evaluation REV] | evaluator-case EVALUATOR_PROFILE --pending [--evaluation-revision REVISION --after-round ROUND]";
    if args.len() < 3
        || !matches!(args[0].as_str(), "evaluator-assess" | "evaluator-case")
        || args[1].is_empty()
        || args[1].starts_with("--")
    {
        return Err(USAGE.into());
    }
    let input = if args[0] == "evaluator-case" && args[2] == "--list" {
        if !matches!(args.len(), 4 | 10)
            || (args.len() == 10
                && (args[4] != "--after"
                    || args[6] != "--at-journal"
                    || args[8] != "--at-evaluation"))
        {
            return Err(USAGE.into());
        }
        let limit = integer(&args[3], false)?;
        if limit > MAX_EVALUATION_CASE_PAGE as u64 {
            return Err("case listing limit exceeds the finite page bound".into());
        }
        let (after_round, expected_basis) = if args.len() == 10 {
            (
                Some(integer(&args[5], false)?),
                Some(FileEvaluationCasesBasis {
                    journal_revision: integer(&args[7], false)?,
                    evaluation_revision: integer(&args[9], true)?,
                }),
            )
        } else {
            (None, None)
        };
        Input::Cases {
            limit: limit as usize,
            after_round,
            expected_basis,
        }
    } else if args[0] == "evaluator-case" && args[2] == "--pending" {
        if !matches!(args.len(), 3 | 7) {
            return Err(USAGE.into());
        }
        let continuation = if args.len() == 3 {
            None
        } else {
            if args[3] != "--evaluation-revision" || args[5] != "--after-round" {
                return Err(USAGE.into());
            }
            Some((integer(&args[4], false)?, integer(&args[6], false)?))
        };
        Input::Pending(continuation)
    } else {
        if args.len() != 3 || args[2].is_empty() || args[2].starts_with("--") {
            return Err(USAGE.into());
        }
        if args[0] == "evaluator-case" {
            Input::Case(integer(&args[2], false)?)
        } else {
            Input::Assessment(Path::new(&args[2]))
        }
    };
    send(Path::new(&args[1]), input, output)
}

fn integer(value: &str, zero: bool) -> Result<u64, String> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.starts_with('0') && (value.len() > 1 || !zero))
    {
        return Err(
            "evaluator arguments require canonical decimal integers in their stated bounds".into(),
        );
    }
    value.parse().map_err(debug)
}

enum Input<'a> {
    Assessment(&'a Path),
    Case(u64),
    Pending(Option<(u64, u64)>),
    Cases {
        limit: usize,
        after_round: Option<u64>,
        expected_basis: Option<FileEvaluationCasesBasis>,
    },
}

enum Outbound {
    Assessment(Request),
    Case(CaseRequest),
    Cases(CasesRequest),
    Pending(PendingRequest),
}

impl Outbound {
    fn wire(&self) -> &[u8] {
        match self {
            Self::Assessment(request) => &request.wire,
            Self::Case(request) => &request.wire,
            Self::Cases(request) => &request.wire,
            Self::Pending(request) => &request.wire,
        }
    }

    fn validate(&self, wire: &[u8]) -> Result<bool, String> {
        match self {
            Self::Assessment(request) => {
                protocol::validate(request, wire).map(|outcome| outcome.completed())
            }
            Self::Case(request) => case::validate(request, wire),
            Self::Cases(request) => cases::validate(request, wire),
            Self::Pending(request) => pending::validate(request, wire),
        }
    }

    fn incomplete(&self) -> &'static str {
        match self {
            Self::Assessment(_) => {
                "assessment refused or completion unconfirmed; inspect the original owner before an explicit retry"
            }
            Self::Case(_) => {
                "original evaluation case is unknown; no case identity or labeling authority was disclosed"
            }
            Self::Cases(_) => {
                "original evaluation case listing is unknown; the pinned history or cursor is unavailable; start an explicit new listing before continuing"
            }
            Self::Pending(_) => {
                "pending evaluation cursor is stale or invalid; explicitly rediscover before selecting another page or label"
            }
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
        Input::Pending(continuation) => {
            Outbound::Pending(PendingRequest::prepare(&settings, continuation)?)
        }
        Input::Cases {
            limit,
            after_round,
            expected_basis,
        } => Outbound::Cases(CasesRequest::prepare(
            &settings,
            limit,
            after_round,
            expected_basis,
        )?),
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
