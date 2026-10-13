//! Runnable stop/drain paths using original file owners and mandatory two-key effects.
use super::*;
#[path = "../../../../tests/support/file_oversight.rs"]
mod fixture;
use fixture::Directory;
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::strict_json::{self, Json, Limits};
use std::{cell::Cell, fs, io, panic::{AssertUnwindSafe, catch_unwind}};

fn configured(root: &Directory) -> Config {
    let mut c = Config::decode(include_bytes!("../../../../fixtures/supervised_publication.json")).unwrap();
    c.store = root.store(); c.programs.clear(); c.timing.runtime_ms = 10000; c
}
fn ordinary(root: &Directory) -> Config {
    let c = configured(root);
    let (mut host, _) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.enable_file_source(host.revision(), c.source_policy).unwrap(); drop(host); c
}
fn effect(root: &Directory, mode: u64) -> Config {
    let mut c = configured(root); c.profile = fixture::profile();
    let (mut host, reviewer) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let keys = fixture::ready(&mut host, &reviewer, 1, b"original reviewed effect");
    if mode > 0 { fixture::dispatch(&mut host, &keys); }
    if mode == 2 { host.publish(host.revision(), 1).unwrap(); }
    drop(keys); drop(reviewer); drop(host); c
}
fn bytes(c: &Config) -> Vec<u8> { fs::read(c.store.join("delivery.bin")).unwrap() }
fn json(out: &[u8]) -> Json { strict_json::parse(out, Limits::default()).unwrap() }
fn stop(c: &Config) -> Json {
    let mut out = Vec::new();
    run_with_elapsed(c, Phase::Admission, 900, || panic!("admission needs no logical clock"),
        &mut || Duration::ZERO, &mut out).unwrap(); json(&out)
}

#[test]
fn offline_admission_stop_reads_no_sources_or_clock_and_reports_no_synthetic_drain() {
    let root = Directory::new(); let c = ordinary(&root);
    let original = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    let value = stop(&c);
    assert_eq!(value.get("status").unwrap().as_str(), Some("admission_stopped"));
    assert_eq!(value.get("drain_attempted").unwrap().as_bool(), Some(false));
    assert_eq!(value.get("appended_events").unwrap().as_u64(), Some(2));
    assert_eq!(value.get("stop_acknowledged").unwrap().as_bool(), Some(true));
    let stopped = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(stopped.revision, original.revision + 2);
    assert_eq!(stopped.control.ledger.elapsed, original.control.ledger.elapsed);
    let saved = bytes(&c); assert_eq!(stop(&c).get("appended_events").unwrap().as_u64(), Some(0));
    assert_eq!(bytes(&c), saved);
}

#[test]
fn explicit_drain_preserves_original_executed_and_expired_outcomes_after_clock_free_stop() {
    for mode in 0..4 {
        let root = Directory::new(); let c = effect(&root, mode);
        let stopped = stop(&c);
        assert_eq!(stopped.get("charged").unwrap().as_str(), Some(if mode > 0 { "16" } else { "0" }));
        assert_eq!(stopped.get("drain_attempted").unwrap().as_bool(), Some(false));
        let mut out = Vec::new();
        let outcome = run_with_elapsed(&c, Phase::Drain, 900,
            || Ok(ElapsedTick(if mode == 3 { 2000 } else { 2 })), &mut || Duration::ZERO, &mut out);
        assert_eq!(outcome.is_ok(), mode != 3);
        let value = json(&out);
        assert_eq!(value.get("charged").unwrap().as_str(), Some(if mode >= 2 { "16" } else { "0" }));
        assert_eq!(value.get("executions").unwrap().as_str(), Some(if mode == 2 { "1" } else { "0" }));
        assert_eq!(value.get("drained").unwrap().as_bool(), Some(mode != 3));
        if mode != 3 {
            let saved = bytes(&c);
            run_with_elapsed(&c, Phase::Drain, 900, || panic!("completed retry needs no clock"),
                &mut || Duration::ZERO, &mut Vec::new()).unwrap();
            assert_eq!(bytes(&c), saved);
        } else {
            assert!(outcome.unwrap_err().contains("unresolved obligations"));
            assert_eq!(value.get("irrecoverable").unwrap().as_array().unwrap().len(), 1);
        }
    }
}

#[test]
fn closed_commands_route_before_shared_options_and_do_not_turn_drain_into_stop() {
    let missing = "/no-such-admission-config".to_owned();
    for arguments in [vec!["stop-admission".into(), missing.clone()],
        vec!["stop-admission".into(), missing.clone(), "0".into()],
        vec!["stop-admission".into(), missing.clone(), "900".into(), "--guarded".into(), "x".into()],
        vec!["drain-stopped".into(), missing, "900".into(), "--credibility-activation".into(), "x".into()]] {
        let error = command_with(&arguments, || panic!("invalid args"), &mut || panic!("no CONFIG admission"), &mut Vec::new()).unwrap_err();
        assert!(error.contains("usage") || error.contains("nonzero"));
        assert!(crate::command(arguments).is_err());
    }
    let root = Directory::new(); let c = ordinary(&root); let before = bytes(&c);
    let mut out = Vec::new();
    assert!(run_with_elapsed(&c, Phase::Drain, 900, || panic!("active drain"), &mut || Duration::ZERO, &mut out).is_err());
    assert!(out.is_empty()); assert_eq!(bytes(&c), before);
    let path = c.store.with_extension("json");
    let text = include_str!("../../../../fixtures/supervised_publication.json")
        .replace("/operator/publication-store", c.store.to_str().unwrap());
    fs::write(&path, text).unwrap();
    let arguments = vec!["stop-admission".into(), path.to_str().unwrap().into(), "900".into()];
    command_with(&arguments, || panic!("parsed stop"), &mut || Duration::ZERO, &mut out).unwrap();
    let stopped = bytes(&c);
    crate::command(arguments).unwrap(); // The actual executable dispatch, exact retry.
    assert_eq!(bytes(&c), stopped);
}

#[test]
fn each_replay_deadline_and_slow_drain_clock_preserve_the_prior_acknowledged_cut() {
    for phase in [Phase::Admission, Phase::Drain] {
        let root = Directory::new(); let c = effect(&root, 1);
        if phase == Phase::Drain { stop(&c); }
        let calls = Cell::new(0);
        run_with_elapsed(&c, phase, 900, || Ok(ElapsedTick(2)), &mut || {
            calls.set(calls.get() + 1); Duration::ZERO
        }, &mut Vec::new()).unwrap();
        for cut in 1..=calls.get() {
            let root = Directory::new(); let c = effect(&root, 1);
            if phase == Phase::Drain { stop(&c); }
            let before = bytes(&c); let staged = c.store.join("delivery.pending"); fs::write(&staged, b"preserve").unwrap();
            let mut count = 0; let mut out = Vec::new();
            let error = run_with_elapsed(&c, phase, 900, || Ok(ElapsedTick(2)), &mut || {
                count += 1; if count >= cut { Duration::from_secs(10) } else { Duration::ZERO }
            }, &mut out).unwrap_err();
            assert!(error.contains("deadline"), "{phase:?} {cut}: {error}");
            assert!(out.is_empty()); assert_eq!(bytes(&c), before); assert_eq!(fs::read(&staged).unwrap(), b"preserve");
        }
    }
}

#[test]
fn clock_failure_and_unwind_never_reopen_the_stopped_domain() {
    for unwind in [false, true] {
        let root = Directory::new(); let c = effect(&root, 1); stop(&c);
        let stopped = bytes(&c);
        let result = catch_unwind(AssertUnwindSafe(|| run_with_elapsed(&c, Phase::Drain, 900, || {
            if unwind { panic!("operator clock failure"); } Err(JournalError::Unavailable)
        }, &mut || Duration::ZERO, &mut Vec::new())));
        if unwind { assert!(result.is_err()); } else { assert!(result.unwrap().is_err()); }
        assert_eq!(bytes(&c), stopped);
        let state = FileOversight::read_publication(&c.store, &c.profile).unwrap();
        assert!(state.stop.is_some()); assert_eq!(state.control.ledger.charged, 16);
        run_with_elapsed(&c, Phase::Drain, 900, || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    }
}

#[test]
fn lost_output_cannot_repeat_stop_or_drain_and_cannot_erase_their_acknowledgments() {
    struct Broken(bool);
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { if self.0 { Ok(bytes.len()) } else { Err(io::ErrorKind::BrokenPipe.into()) } }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    for phase in [Phase::Admission, Phase::Drain] { for flush in [false, true] {
        let root = Directory::new(); let c = effect(&root, 1);
        if phase == Phase::Drain { stop(&c); }
        let error = run_with_elapsed(&c, phase, 900, || Ok(ElapsedTick(2)),
            &mut || Duration::ZERO, &mut Broken(flush)).unwrap_err();
        assert!(error.contains("acknowledged")); assert!(error.contains("output failed"));
        let saved = bytes(&c);
        run_with_elapsed(&c, phase, 900, || panic!("no time on exact retry"),
            &mut || Duration::ZERO, &mut Vec::new()).unwrap();
        assert_eq!(bytes(&c), saved);
    }}
}

#[test]
fn the_runnable_two_phase_path_uses_only_the_original_terminal_reserve() {
    let sizing = Directory::new(); let c = effect(&sizing, 1);
    let count = FileOversight::read_publication(&c.store, &c.profile).unwrap().revision as usize;
    let root = Directory::new(); let mut c = configured(&root); c.profile = fixture::profile();
    c.profile.delivery.limits.events = count + 3;
    let (mut host, reviewer) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let keys = fixture::ready(&mut host, &reviewer, 1, b"original reviewed effect");
    fixture::dispatch(&mut host, &keys);
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    drop(keys); drop(reviewer); drop(host);
    stop(&c);
    assert_eq!(FileOversight::read_publication(&c.store, &c.profile).unwrap().revision as usize, count + 2);
    run_with_elapsed(&c, Phase::Drain, 900, || Ok(ElapsedTick(2)), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    assert_eq!(FileOversight::read_publication(&c.store, &c.profile).unwrap().revision as usize, count + 3);
    let saved = bytes(&c); stop(&c);
    run_with_elapsed(&c, Phase::Drain, 900, || panic!("full exact retry"), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    assert_eq!(bytes(&c), saved);
}
