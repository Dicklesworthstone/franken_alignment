//! Original Store barriers: a timer cannot report unacknowledged containment.
use super::*;
use crate::action::consequence::oversight::consistency::ConsistencyStopPolicy;

fn configured(root: &Directory, limit: usize) -> (FileOversight, FileConsistencyObserver) {
    let mut p = profile(); p.delivery.limits.events = limit;
    let (mut host, _) = FileOversight::create(root.store(), p).unwrap();
    let config = FileConsistencyConfig::new(parameters()).unwrap()
        .with_terminal_stop(ConsistencyStopPolicy::new(19, 1, 7007).unwrap()).unwrap();
    let role = host.enable_action_consistency(0, config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    predict(&mut host, &role).unwrap().unwrap();
    (host, role)
}

#[test]
fn every_expiry_storage_barrier_preserves_either_pending_history_or_committed_stop() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut host, role) = configured(&root, 4096);
        let deadline = host.action_consistency_deadline().unwrap().unwrap();
        let before = host.inspect(); let evidence = host.action_consistency_snapshot().unwrap().evidence;
        host.store.fail_once(stage); let r = host.revision();
        check_failure(role.expire_forecast(&mut host, r, deadline, deadline.expires_at).unwrap_err(), stage);
        assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_some());
        assert_eq!(host.action_consistency_deadline(), Err(JournalError::Unavailable));
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(disk.stop.is_some(), stage == JournalIo::DirectorySync);
        assert_eq!(disk.revision, before.revision + u64::from(stage == JournalIo::DirectorySync));
        drop(host);
        let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
        let state = host.action_consistency_snapshot().unwrap();
        // A canonical expiry preserves its stop; an unrecorded expiry leaves an
        // unanswered forecast, which the ORIGINAL recovery treats as lost coverage.
        assert!(state.coverage_lost); assert_eq!(state.pending_attempt, Some(1));
        assert_eq!(state.evidence, evidence); assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().stop.unwrap().request().operation, 7007);
        assert!(!host.clock_ready());
    }
}

#[test]
fn due_timer_disables_before_capacity_but_early_and_stale_timers_do_not() {
    for limit in [3, 4] {
        let root = Directory::new(); let (mut host, role) = configured(&root, limit);
        let deadline = host.action_consistency_deadline().unwrap().unwrap();
        let before = host.inspect(); let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
        let r = host.revision();
        assert_eq!(role.expire_forecast(&mut host, r, deadline, ElapsedTick(10)), Ok(Ok(false)));
        let mut wrong = deadline; wrong.source_sequence += 1;
        assert_eq!(role.expire_forecast(&mut host, r, wrong, ElapsedTick(11)), Err(Error::Stale.into()));
        assert!(host.storage_failure().is_none()); assert_eq!(host.inspect(), before);
        assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
        let result = role.expire_forecast(&mut host, r, deadline, ElapsedTick(11));
        if limit == 3 {
            assert_eq!(result, Err(Error::Limit.into())); assert!(host.storage_failure().is_some());
            assert_eq!(host.inspect(), before);
            assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
        } else {
            assert_eq!(result, Ok(Ok(true))); assert_eq!(host.revision(), 4);
            assert!(host.inspect().stop.is_some());
        }
    }
}

#[test]
fn expiry_codec_and_replay_reject_incomplete_or_early_timers_without_changing_old_time_semantics() {
    let root = Directory::new(); let (mut host, role) = configured(&root, 4096);
    let d = host.action_consistency_deadline().unwrap().unwrap();
    let event = ConsistencyEvent::Expire(d, ElapsedTick(11));
    let mut w = super::super::super::super::codec::shared::Writer::new(100);
    write(&mut w, &event).unwrap(); let encoded = w.finish();
    let mut golden = vec![6];
    for value in [1_u64, 0, 0, 1, 1, 11, 11] { golden.extend_from_slice(&value.to_be_bytes()); }
    assert_eq!(encoded, golden);
    for end in 0..encoded.len() {
        assert!(read(&mut super::super::super::super::codec::shared::Reader::new(&encoded[..end])).is_err());
    }
    let mut r = super::super::super::super::codec::shared::Reader::new(&encoded);
    let decoded = read(&mut r).unwrap(); r.end().unwrap();
    let mut events = host.events.clone();
    events.push(Event::Consistency(ConsistencyEvent::Expire(d, ElapsedTick(10))));
    assert!(Machine::replay(&host.profile, &events).is_err());
    events.pop(); let mut altered = d; altered.authority_epoch += 1;
    events.push(Event::Consistency(ConsistencyEvent::Expire(altered, ElapsedTick(11))));
    assert!(Machine::replay(&host.profile, &events).is_err());
    events.pop(); events.push(Event::Consistency(decoded));
    assert!(Machine::replay(&host.profile, &events).unwrap().snapshot(events.len()).stop.is_some());
    // Old time records do NOT acquire new implicit expiry semantics on replay.
    host.observe_time(host.revision(), ElapsedTick(11)).unwrap();
    assert!(host.inspect().stop.is_none()); assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
    host.source_interrupted = true; let r = host.revision();
    assert_eq!(role.expire_forecast(&mut host, r, d, ElapsedTick(11)), Ok(Ok(true)));
    assert!(host.source_interrupted);
    host.observe_time(host.revision(), ElapsedTick(12)).unwrap(); let before = host.inspect(); let r = host.revision();
    assert_eq!(role.expire_forecast(&mut host, r, d, ElapsedTick(11)), Ok(Ok(true)));
    assert_eq!(host.inspect(), before); assert!(host.source_interrupted);
}
