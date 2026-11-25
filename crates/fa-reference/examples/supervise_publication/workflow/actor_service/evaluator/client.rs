//! Explicit authenticated case reads and assessments; bounded inventory walks
//! share one original invocation allowance and never restart or retry a cursor.
#[path = "client/inventory.rs"]
mod inventory;

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
use fa_reference::action::consequence::oversight::actor_peer::PeerCredentials;
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
    const USAGE: &str = "usage: evaluator-assess EVALUATOR_PROFILE ASSESSMENT_JSON | evaluator-case EVALUATOR_PROFILE ROUND | evaluator-case EVALUATOR_PROFILE --list LIMIT [--after ROUND --at-journal REV --at-evaluation REV | --max-pages 1..512] | evaluator-case EVALUATOR_PROFILE --pending [--evaluation-revision REVISION --after-round ROUND | --max-pages 1..32]";
    if args.len() < 3
        || !matches!(args[0].as_str(), "evaluator-assess" | "evaluator-case")
        || args[1].is_empty()
        || args[1].starts_with("--")
    {
        return Err(USAGE.into());
    }
    let mut max_pages = None;
    let input = if args[0] == "evaluator-case" && args[2] == "--list" {
        if !matches!(args.len(), 4 | 6 | 10)
            || (args.len() == 6 && args[4] != "--max-pages")
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
        if args.len() == 6 {
            max_pages = Some(inventory_pages(&args[5], inventory::MAX_CASE_PAGES)?);
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
        if !matches!(args.len(), 3 | 5 | 7)
            || (args.len() == 5 && args[3] != "--max-pages")
        {
            return Err(USAGE.into());
        }
        if args.len() == 5 {
            max_pages = Some(inventory_pages(&args[4], inventory::MAX_PENDING_PAGES)?);
        }
        let continuation = if args.len() != 7 {
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
    send(Path::new(&args[1]), input, max_pages, output)
}

fn inventory_pages(value: &str, maximum: usize) -> Result<usize, String> {
    let pages = integer(value, false)?;
    if pages > maximum as u64 {
        return Err("evaluator inventory page bound exceeds its finite native inventory".into());
    }
    Ok(pages as usize)
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

fn send(
    profile: &Path,
    input: Input<'_>,
    max_pages: Option<usize>,
    output: &mut impl Write,
) -> Result<(), String> {
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
    if let Some(max_pages) = max_pages {
        return inventory::send(&settings, request, max_pages, &mut checkpoint, output);
    }
    let mut original_peer = None;
    let receipt = exchange(&settings, &request, &mut original_peer, &mut checkpoint)?;
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

/// One EOF-sealed exchange. An explicit inventory walk pins the first actual
/// kernel peer, even when the configured policy permits more than one PID.
/// No later process receives a continuation merely by sharing UID/GID.
fn exchange(
    settings: &Settings,
    request: &Outbound,
    original_peer: &mut Option<PeerCredentials>,
    checkpoint: &mut impl FnMut() -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    checkpoint()?;
    let mut socket = UnixStream::connect(&settings.socket).map_err(debug)?;
    checkpoint()?;
    let observed = settings
        .supervisor
        .policy()?
        .verify(&socket)
        .map_err(|_| "unverified supervisor; no evaluator request bytes disclosed")?;
    if original_peer.is_some_and(|original| original != observed) {
        return Err("original evaluator supervisor changed between pages; no continuation bytes disclosed".into());
    }
    *original_peer = Some(observed);
    checkpoint()?;
    let mut wait = || std::thread::sleep(Duration::from_millis(settings.poll_ms));
    transport::write(&mut socket, request.wire(), checkpoint, &mut wait)?;
    checkpoint()?;
    socket.shutdown(Shutdown::Write).map_err(debug)?;
    transport::read(
        &mut socket,
        protocol::RESPONSE_BYTES,
        checkpoint,
        &mut wait,
    )
}
