//! Cross-domain behavior through the actual operator loop and original durable
//! stop owners. Helper ballots in the reused fixture are synthetic control inputs.
use super::*;
use fa_reference::strict_json::{self, Json, Limits};
use std::fs;
use std::io::{self, Write};
#[path = "../../../tests/support/file_oversight.rs"]
mod fixture;
#[path = "fixtures.rs"]
mod support;
use support::*;

fn parsed(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }
fn records(json: &Json) -> &[Json] { json.get("members").unwrap().as_array().unwrap() }
fn receipt(report: &Report, index: usize) -> StopControlReceipt {
    match report.members[index].outcome { Outcome::Native(receipt) => receipt, other => panic!("native receipt required: {other:?}") }
}
fn assert_untouched(f: &Fixture) {
    for server in f.servers.borrow().iter() {
        assert_eq!(server.applied, 0);
        assert_eq!(server.image(), server.before);
        assert!(server.driver.supervisor().host().unwrap().inspect().stop.is_none());
        server.assert_locked();
    }
}

#[test]
fn actual_command_stops_locked_live_domains_and_preserves_real_effect_accounting() {
    let f = Fixture::new(&[(Behavior::Normal, Seed::Reserved), (Behavior::Normal, Seed::Sent),
        (Behavior::Normal, Seed::Published)]);
    for server in f.servers.borrow().iter() { server.assert_locked(); }
    let mut out = Vec::new();
    f.command(&mut out).unwrap();
    let json = parsed(&out);
    assert_eq!(json.get("acknowledged").unwrap().as_bool(), Some(true));
    assert_eq!(json.get("drained").unwrap().as_bool(), Some(true));
    assert_eq!(json.get("deadline_reached").unwrap().as_bool(), Some(false));
    assert_eq!(records(&json).len(), 3);
    for (index, server) in f.servers.borrow().iter().enumerate() {
        let host = server.driver.supervisor().host().unwrap();
        let state = host.inspect();
        let native = state.stop.unwrap();
        assert_eq!(native.request().operation, server.operation);
        assert!(host.stop_progress().unwrap().drained());
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(state.control.ledger.charged, if index == 2 { 16 } else { 0 });
        assert_eq!(state.executions, u64::from(index == 2));
        assert_eq!(server.accepted, 1); assert_eq!(server.applied, 1);
        assert_ne!(server.image(), server.before);
        assert_eq!(server.image(), *server.applied_image.as_ref().unwrap());
        assert_eq!(records(&json)[index].get("control_sequence").unwrap().as_str(),
            Some(native.control_sequence().to_string().as_str()));
        assert_eq!(records(&json)[index].get("operation").unwrap().as_str(),
            Some(server.operation.to_string().as_str()));
        assert_eq!(records(&json)[index].get("status").unwrap().as_str(), Some("stopped_drained"));
        server.assert_locked();
    }
}

#[test]
fn connected_silent_or_malformed_member_cannot_starve_a_later_live_domain() {
    for behavior in [Behavior::Silent, Behavior::Malformed] {
        let f = Fixture::new(&[(behavior, Seed::None), (Behavior::Normal, Seed::Reserved)]);
        let mut plan = f.plan();
        plan.runtime = Duration::from_millis(200);
        plan.members[0].peer.runtime_ms = 7;
        let report = f.drive(&plan);
        assert!(!report.acknowledged()); assert!(!report.drained());
        assert!(!report.expired, "only the first member's own timeout should expire");
        assert_eq!(report.members.len(), 2);
        assert_eq!(report.members[0].outcome, Outcome::Unconfirmed {
            reason: if behavior == Behavior::Silent { Reason::Deadline } else { Reason::Protocol },
            may_have_been_sent: false,
        });
        assert!(receipt(&report, 1).drained());
        let servers = f.servers.borrow();
        assert_eq!(servers[0].applied, 0); assert_eq!(servers[0].image(), servers[0].before);
        assert_eq!(servers[1].applied, 1);
        assert!(servers[1].driver.supervisor().host().unwrap().stop_progress().unwrap().drained());
        for server in servers.iter() { assert_eq!(server.accepted, 1); server.assert_locked(); }
    }
}

#[test]
fn lost_native_receipt_stays_unconfirmed_without_reconnect_or_unknown_refund() {
    let f = Fixture::new(&[(Behavior::LostReceipt, Seed::Sent), (Behavior::Normal, Seed::Reserved)]);
    f.servers.borrow_mut()[0].tick = fa_reference::action::ElapsedTick(2000);
    let report = f.drive(&f.plan());
    assert_eq!(report.members[0].outcome,
        Outcome::Unconfirmed { reason: Reason::Protocol, may_have_been_sent: true });
    assert!(receipt(&report, 1).drained());
    assert!(!report.acknowledged()); assert!(!report.drained());
    let servers = f.servers.borrow();
    let first = &servers[0];
    let host = first.driver.supervisor().host().unwrap();
    assert!(host.inspect().stop.is_some());
    assert!(!host.stop_progress().unwrap().drained());
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(first.accepted, 1); assert_eq!(first.applied, 1);
    let stopped = first.image();
    assert_eq!(stopped, *first.applied_image.as_ref().unwrap());
    let mut out = Vec::new();
    write_report(&report, &mut out).unwrap();
    assert_eq!(records(&parsed(&out))[0].get("request_may_have_been_sent").unwrap().as_bool(), Some(true));
    assert_eq!(first.image(), stopped, "rendering cannot reconnect or apply a second stop");
}

#[test]
fn native_pending_and_refused_drain_are_acknowledged_without_claiming_completion() {
    let f = Fixture::new(&[(Behavior::Normal, Seed::Sent), (Behavior::Normal, Seed::None),
        (Behavior::Normal, Seed::Reserved)]);
    {
        let mut servers = f.servers.borrow_mut();
        servers[0].tick = fa_reference::action::ElapsedTick(2000);
        servers[1].tick = fa_reference::action::ElapsedTick(0);
    }
    let mut out = Vec::new();
    assert!(f.command(&mut out).is_err());
    let json = parsed(&out);
    assert_eq!(json.get("acknowledged").unwrap().as_bool(), Some(true));
    assert_eq!(json.get("drained").unwrap().as_bool(), Some(false));
    assert_eq!(records(&json)[0].get("status").unwrap().as_str(), Some("stopped_pending"));
    assert_eq!(records(&json)[1].get("status").unwrap().as_str(), Some("stopped_drain_refused"));
    assert_eq!(records(&json)[2].get("status").unwrap().as_str(), Some("stopped_drained"));
    for server in f.servers.borrow().iter() {
        assert!(server.driver.supervisor().host().unwrap().inspect().stop.is_some());
        assert_eq!(server.applied, 1); assert_eq!(server.accepted, 1);
    }
    assert_eq!(f.servers.borrow()[0].driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn full_roster_and_closed_profile_admission_precede_every_connection() {
    let f = Fixture::new(&[(Behavior::Normal, Seed::None), (Behavior::Normal, Seed::None)]);
    let original = f.document();
    let prefix = &original[..original.find("\"members\":[").unwrap()];
    let first_path = f.servers.borrow()[0].peer_path.clone();
    let entry = format!("{{\"id\":\"1\",\"operation\":\"1001\",\"peer_profile\":\"{}\"}}", first_path.display());
    for invalid in [
        format!("{prefix}\"members\":[]}}"),
        format!("{prefix}\"members\":[{}]}}", vec![entry; 65].join(",")),
        original.replace("stop/1", "stop/2"),
        original.replace("\"steps_per_peer\":2048", "\"steps_per_peer\":0"),
        original.replace("\"steps_per_peer\":2048", "\"steps_per_peer\":65537"),
        original.replace("\"runtime_ms\":1000", "\"runtime_ms\":0"),
        original.replace("\"id\":\"1\"", "\"id\":\"01\""),
        original.replace("\"id\":\"2\"", "\"id\":\"1\""),
        original.replace("\"operation\":\"1001\"", "\"operation\":\"0\""),
        original.replace("\"members\":[", "\"unknown\":true,\"members\":["),
        original.replace("peer-2.json", "missing-profile.json"),
    ] {
        private(&f.manifest(), invalid.as_bytes());
        let mut out = Vec::new();
        assert!(f.command(&mut out).is_err());
        assert!(out.is_empty()); assert_untouched(&f);
        assert!(f.servers.borrow().iter().all(|server| server.accepted == 0));
    }
    private(&f.manifest(), original.as_bytes());
    let peer_path = f.servers.borrow()[1].peer_path.clone();
    let saved = fs::read_to_string(&peer_path).unwrap();
    private(&peer_path, saved.replace("\"run\":2", "\"run\":1")
        .replace("\"authority\":102", "\"authority\":101").as_bytes());
    assert!(f.command(&mut Vec::new()).unwrap_err().contains("duplicate fleet authority"));
    private(&peer_path, saved.replace("/p2\"", "/p1\"").as_bytes());
    private(&f.manifest(), original.replace("\"operation\":\"1002\"", "\"operation\":\"1001\"").as_bytes());
    assert!(f.command(&mut Vec::new()).unwrap_err().contains("duplicate fleet stop endpoint"));
    assert_untouched(&f);
    assert!(f.servers.borrow().iter().all(|server| server.accepted == 0));
    private(&peer_path, saved.as_bytes());
    private(&f.manifest(), original.as_bytes());
    f.command(&mut Vec::new()).unwrap();
}

#[test]
fn wrong_authenticated_audience_operation_or_server_pid_never_sends_a_stop() {
    for changed in 0..3 {
        let f = Fixture::new(&[(Behavior::Normal, Seed::None), (Behavior::Normal, Seed::Reserved)]);
        let path = f.servers.borrow()[0].peer_path.clone();
        let original = fs::read_to_string(&path).unwrap();
        if changed == 0 {
            private(&path, original.replace("\"authority\":101", "\"authority\":201").as_bytes());
        } else if changed == 1 {
            // The original socket now offers a different operation. The client
            // must reject the offer instead of following the server's choice.
            f.servers.borrow_mut()[0].operation += 100;
        } else {
            let pid = std::process::id();
            private(&path, original.replacen(&format!("\"pid\":{pid}"),
                &format!("\"pid\":{}", pid.checked_add(1).unwrap()), 1).as_bytes());
        }
        let report = f.drive(&f.plan());
        assert_eq!(report.members[0].outcome, Outcome::Unconfirmed {
            reason: if changed == 2 { Reason::Connection } else { Reason::Protocol },
            may_have_been_sent: false,
        });
        assert!(receipt(&report, 1).drained());
        let servers = f.servers.borrow();
        assert_eq!(servers[0].applied, 0); assert_eq!(servers[0].image(), servers[0].before);
        assert_eq!(servers[1].applied, 1);
    }
}

#[test]
fn global_connect_and_offer_deadlines_keep_the_complete_roster_without_sending_late_requests() {
    for after_connect in [false, true] {
        let f = Fixture::new(&[(Behavior::Normal, Seed::None), (Behavior::Normal, Seed::None)]);
        let plan = f.plan();
        let mut calls = 0;
        let report = drive(&plan, &mut || {
            calls += 1;
            let now = f.elapsed();
            // Two connected members consume four checks. Check six is AFTER
            // the first complete native offer and before request_stop().
            if !after_connect && calls >= 6 { Duration::from_secs(1) } else { now }
        }, &mut |member: &Member| {
            let client = member.peer.connect_stop_client(member.operation)?;
            if after_connect { f.ticks.set(1000); }
            Ok::<_, String>(client)
        }, &mut |delay| f.pause(delay)).unwrap();
        assert!(report.expired); assert_eq!(report.members.len(), 2);
        assert_eq!(report.members[0].outcome,
            Outcome::Unconfirmed { reason: Reason::Deadline, may_have_been_sent: false });
        if after_connect {
            assert_eq!(report.members[1].outcome, Outcome::NotAttempted);
            assert!(!report.members[1].connection_attempted);
        } else {
            assert_eq!(report.members[0].steps, 1);
            assert_eq!(report.members[1].steps, 0);
            assert_eq!(report.members[1].outcome,
                Outcome::Unconfirmed { reason: Reason::Deadline, may_have_been_sent: false });
        }
        assert_untouched(&f);
    }
}

#[test]
fn a_final_native_receipt_crossing_the_deadline_remains_factual_but_is_not_timely_success() {
    let control = Fixture::new(&[(Behavior::Normal, Seed::Reserved), (Behavior::Normal, Seed::Reserved)]);
    control.command(&mut Vec::new()).unwrap();
    let last_check = control.checks.get();
    let f = Fixture::new(&[(Behavior::Normal, Seed::Reserved), (Behavior::Normal, Seed::Reserved)]);
    let mut out = Vec::new();
    let result = command_with(&f.args(), &mut out, &mut || {
        let now = f.elapsed();
        if f.checks.get() >= last_check { Duration::from_secs(1) } else { now }
    }, &mut |delay| f.pause(delay));
    assert!(result.is_err());
    let json = parsed(&out);
    assert_eq!(json.get("acknowledged").unwrap().as_bool(), Some(true));
    assert_eq!(json.get("drained").unwrap().as_bool(), Some(true));
    assert_eq!(json.get("deadline_reached").unwrap().as_bool(), Some(true));
    assert_eq!(records(&json)[1].get("deadline_reached").unwrap().as_bool(), Some(true));
    for server in f.servers.borrow().iter() {
        assert_eq!(server.applied, 1);
        assert_eq!(server.image(), *server.applied_image.as_ref().unwrap());
    }
}

#[test]
fn exact_client_step_budget_is_enforced_without_renewal_or_an_extra_request() {
    let control = Fixture::new(&[(Behavior::Normal, Seed::Reserved)]);
    let actual = control.drive(&control.plan());
    let exact = actual.members[0].steps;
    assert!(exact > 1); assert!(actual.drained());
    let f = Fixture::new(&[(Behavior::Normal, Seed::Reserved)]);
    let mut plan = f.plan(); plan.steps_per_peer = exact;
    assert!(f.drive(&plan).drained());
    let f = Fixture::new(&[(Behavior::Normal, Seed::Reserved)]);
    let mut plan = f.plan(); plan.steps_per_peer = 1;
    let report = f.drive(&plan);
    assert_eq!(report.members[0].steps, 1);
    assert_eq!(report.members[0].outcome,
        Outcome::Unconfirmed { reason: Reason::StepLimit, may_have_been_sent: false });
    assert_untouched(&f);
}

#[test]
fn report_write_or_flush_failure_never_reconnects_or_reapplies_completed_stops() {
    struct Broken(bool);
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0 { Ok(bytes.len()) } else { Err(io::ErrorKind::BrokenPipe.into()) }
        }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    for flush_only in [false, true] {
        let f = Fixture::new(&[(Behavior::Normal, Seed::Reserved), (Behavior::Normal, Seed::Sent)]);
        let error = f.command(&mut Broken(flush_only)).unwrap_err();
        assert!(error.contains("after control work"));
        for server in f.servers.borrow().iter() {
            assert_eq!(server.accepted, 1); assert_eq!(server.applied, 1);
            assert_eq!(server.image(), *server.applied_image.as_ref().unwrap());
            assert!(server.driver.supervisor().host().unwrap().stop_progress().unwrap().drained());
            server.assert_locked();
        }
    }
}
