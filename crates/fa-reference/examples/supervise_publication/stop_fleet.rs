//! One stop request per independently configured live member through the original
//! authenticated protocol. This is not a distributed fence or durable frontier.
//! Unix connect and file intake are synchronous; only CONNECTED peers are polled
//! fairly. A blocked kernel connect cannot be preempted by this operator.
use std::io::Write;

#[cfg(not(target_os = "linux"))]
pub fn command(_args: &[String], _out: &mut impl Write) -> Result<(), String> {
    Err("stop-fleet requires Linux peer credentials; no unchecked fallback".into())
}

#[cfg(target_os = "linux")]
#[path = "stop_fleet/profile.rs"]
mod profile;
#[cfg(target_os = "linux")]
use profile::{Member, Plan};
#[cfg(target_os = "linux")]
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::{
    StopClient, StopClientProgress, StopControlReceipt, StopStatus,
};
#[cfg(target_os = "linux")]
use std::io::Read;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
pub fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let started = Instant::now();
    command_with(args, out, &mut || started.elapsed(), &mut std::thread::sleep)
}

#[cfg(target_os = "linux")]
fn command_with<T: FnMut() -> Duration, P: FnMut(Duration)>(
    args: &[String], out: &mut impl Write, elapsed: &mut T, pause: &mut P,
) -> Result<(), String> {
    if args.len() != 2 || args[0] != "stop-fleet" {
        return Err("usage: supervise_publication stop-fleet FLEET_JSON".into());
    }
    let plan = Plan::read(std::path::Path::new(&args[1]), elapsed)?;
    let report = drive(&plan, elapsed, &mut |member: &Member| {
        member.peer.connect_stop_client(member.operation)
    }, pause)?;
    let complete = report.drained() && !report.expired
        && report.members.iter().all(|member| !member.deadline_reached);
    write_report(&report, out).map_err(|error| format!(
        "stop-fleet report output failed after control work; inspect original member histories before any retry: {error}"
    ))?;
    if complete { Ok(()) } else {
        Err("stop-fleet did not finish within its bounds with every member drained; inspect each member outcome and original recovery state before any retry".into())
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reason { Connection, Protocol, Deadline, StepLimit }
#[cfg(target_os = "linux")]
impl Reason {
    fn label(self) -> &'static str {
        match self { Self::Connection => "connection", Self::Protocol => "protocol",
            Self::Deadline => "deadline", Self::StepLimit => "step_limit" }
    }
}
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    NotAttempted,
    Unconfirmed { reason: Reason, may_have_been_sent: bool },
    Native(StopControlReceipt),
}
#[cfg(target_os = "linux")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct MemberReport {
    id: u64,
    operation: u64,
    expected: fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::ReviewerExpectation,
    connection_attempted: bool,
    deadline_reached: bool,
    steps: usize,
    outcome: Outcome,
}
#[cfg(target_os = "linux")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct Report { members: Vec<MemberReport>, expired: bool }
#[cfg(target_os = "linux")]
impl Report {
    fn acknowledged(&self) -> bool {
        !self.members.is_empty() && self.members.iter().all(|member|
            matches!(member.outcome, Outcome::Native(receipt) if receipt.acknowledged()))
    }
    fn drained(&self) -> bool {
        self.acknowledged() && self.members.iter().all(|member|
            matches!(member.outcome, Outcome::Native(receipt) if receipt.drained()))
    }
}
#[cfg(target_os = "linux")]
struct Pending<S> { client: Option<StopClient<S>>, started: Duration }
#[cfg(target_os = "linux")]
fn refuse<S: Read + Write>(pending: &mut Pending<S>, member: &mut MemberReport, reason: Reason) {
    let may_have_been_sent = pending.client.as_ref().is_some_and(StopClient::outcome_unknown);
    member.outcome = Outcome::Unconfirmed { reason, may_have_been_sent };
    member.deadline_reached |= reason == Reason::Deadline;
    // A deadline, malformed response or failed send never reconnects a member.
    pending.client = None;
}

/// The full roster and all result/state slots are admitted before any connection.
/// No native stop can be inferred from admission, connection or transmission.
/// Each connected peer gets at most one original step per round; failed peers
/// stay terminal while other members retain their ORIGINAL budgets and deadlines.
#[cfg(target_os = "linux")]
fn drive<S, T, C, P>(plan: &Plan, elapsed: &mut T, connect: &mut C, pause: &mut P)
    -> Result<Report, String>
where S: Read + Write, T: FnMut() -> Duration,
    C: FnMut(&Member) -> Result<StopClient<S>, String>, P: FnMut(Duration)
{
    let mut members = Vec::new();
    members.try_reserve_exact(plan.members.len()).map_err(|_| "fleet result capacity refused")?;
    let mut pending = Vec::new();
    pending.try_reserve_exact(plan.members.len()).map_err(|_| "fleet client capacity refused")?;
    for member in &plan.members {
        members.push(MemberReport { id: member.id, operation: member.operation,
            expected: member.peer.expected, connection_attempted: false, deadline_reached: false, steps: 0,
            outcome: Outcome::NotAttempted });
        pending.push(Pending { client: None, started: Duration::ZERO });
    }
    let mut report = Report { members, expired: false };
    for ((member, result), active) in plan.members.iter().zip(&mut report.members).zip(&mut pending) {
        let now = elapsed();
        if now >= plan.runtime { report.expired = true; break; }
        // The synchronous connect is attempted ONCE. Its time is charged to this
        // member too; successful return cannot reset the global elapsed budget.
        result.connection_attempted = true;
        active.started = now;
        match connect(member) {
            Ok(client) => active.client = Some(client),
            Err(_) => {
                result.outcome = Outcome::Unconfirmed { reason: Reason::Connection, may_have_been_sent: false };
            }
        }
        let now = elapsed();
        if now >= plan.runtime { report.expired = true; break; }
        if now.saturating_sub(active.started) >= Duration::from_millis(member.peer.runtime_ms)
            && active.client.is_some()
        { refuse(active, result, Reason::Deadline); }
    }
    while pending.iter().any(|slot| slot.client.is_some()) && !report.expired {
        let mut progressed = false;
        for ((member, result), active) in plan.members.iter().zip(&mut report.members).zip(&mut pending) {
            if active.client.is_none() { continue; }
            let now = elapsed();
            if now >= plan.runtime { report.expired = true; break; }
            if now.saturating_sub(active.started) >= Duration::from_millis(member.peer.runtime_ms) {
                refuse(active, result, Reason::Deadline); continue;
            }
            if result.steps >= plan.steps_per_peer {
                refuse(active, result, Reason::StepLimit); continue;
            }
            result.steps += 1;
            let step = active.client.as_mut().expect("active client").step();
            // A completed native read/write cannot hide time spent in that
            // step. In particular, no new decision is queued after expiry.
            let after = elapsed();
            report.expired |= after >= plan.runtime;
            let expired = report.expired || after.saturating_sub(active.started)
                >= Duration::from_millis(member.peer.runtime_ms);
            result.deadline_reached |= expired;
            match step {
                Ok(StopClientProgress::Complete) => {
                    // A valid received receipt is a historical fact even when
                    // the step crossed its deadline. Keep it and expose expiry.
                    match active.client.as_ref().and_then(StopClient::receipt) {
                        Some(receipt) => { result.outcome = Outcome::Native(receipt); active.client = None; }
                        None => refuse(active, result, Reason::Protocol),
                    }
                    progressed = true;
                }
                Err(_) => { refuse(active, result, Reason::Protocol); progressed = true; }
                Ok(_) if expired => refuse(active, result, Reason::Deadline),
                Ok(StopClientProgress::NeedsDecision) => {
                    // Invoking stop-fleet is the explicit restrictive choice.
                    // The native client checks the complete offer before this.
                    if active.client.as_mut().expect("active client").request_stop().is_err() {
                        refuse(active, result, Reason::Protocol);
                    }
                    progressed = true;
                }
                Ok(StopClientProgress::Progress) => progressed = true,
                Ok(StopClientProgress::Blocked) => {}
            }
            if report.expired { break; }
        }
        if !progressed && !report.expired && pending.iter().any(|slot| slot.client.is_some()) {
            let now = elapsed();
            if now >= plan.runtime { report.expired = true; break; }
            let mut delay = plan.poll.min(plan.runtime.saturating_sub(now));
            for (member, active) in plan.members.iter().zip(&pending) {
                if active.client.is_some() {
                    delay = delay.min(Duration::from_millis(member.peer.poll_ms));
                    delay = delay.min(Duration::from_millis(member.peer.runtime_ms)
                        .saturating_sub(now.saturating_sub(active.started)));
                }
            }
            if !delay.is_zero() { pause(delay); }
        }
    }
    if report.expired {
        for (result, active) in report.members.iter_mut().zip(&mut pending) {
            if active.client.is_some() { refuse(active, result, Reason::Deadline); }
            else if !result.connection_attempted { result.deadline_reached = true; }
        }
    }
    Ok(report)
}

#[cfg(target_os = "linux")]
fn write_report(report: &Report, out: &mut impl Write) -> std::io::Result<()> {
    write!(out, "{{\"schema_version\":\"fa.live-fleet-stop-result/1\",\"acknowledged\":{},\"drained\":{},\"deadline_reached\":{},\"members\":[",
        report.acknowledged(), report.drained(), report.expired)?;
    for (index, member) in report.members.iter().enumerate() {
        if index != 0 { write!(out, ",")?; }
        let (status, reason, sent, receipt) = match member.outcome {
            Outcome::NotAttempted => ("not_attempted", "deadline", false, None),
            Outcome::Unconfirmed { reason, may_have_been_sent } =>
                ("unconfirmed", reason.label(), may_have_been_sent, None),
            Outcome::Native(receipt) => {
                let status = match receipt.status {
                    StopStatus::Unconfirmed => "unconfirmed",
                    StopStatus::StoppedDrainRefused => "stopped_drain_refused",
                    StopStatus::StoppedPending => "stopped_pending",
                    StopStatus::StoppedDrained => "stopped_drained",
                };
                (status, if receipt.acknowledged() { "none" } else { "native_refusal" }, true, Some(receipt))
            }
        };
        let scope = member.expected.scope;
        write!(out, "{{\"id\":\"{}\",\"operation\":\"{}\",\"scope\":{{\"tenant\":\"{}\",\"principal\":\"{}\",\"run\":\"{}\",\"branch\":\"{}\",\"authority\":\"{}\"}},\"reviewer_id\":\"{}\",\"clock_domain\":\"{}\",\"connection_attempted\":{},\"deadline_reached\":{},\"steps\":{},\"status\":\"{status}\",\"reason\":\"{reason}\",\"request_may_have_been_sent\":{sent},\"acknowledged\":{},\"drained\":{}",
            member.id, member.operation, scope.tenant, scope.principal, scope.run, scope.branch,
            scope.authority, member.expected.reviewer, member.expected.clock_domain,
            member.connection_attempted, member.deadline_reached, member.steps,
            receipt.is_some_and(StopControlReceipt::acknowledged), receipt.is_some_and(StopControlReceipt::drained))?;
        if let Some(receipt) = receipt {
            write!(out, ",\"control_sequence\":\"{}\",\"revocation_floor\":\"{}\",\"dispatcher_epoch\":\"{}\"",
                receipt.control_sequence, receipt.revocation_floor, receipt.dispatcher_epoch)?;
        }
        write!(out, "}}")?;
    }
    writeln!(out, "]}}")?;
    out.flush()
}

#[cfg(all(test, target_os = "linux"))]
#[path = "stop_fleet/tests.rs"]
mod tests;
