//! Exact composition at a pending human decision and the post-dispatch boundary.
use super::*;
use std::sync::{Arc, atomic::AtomicBool};
fn wait_flag(flag: &AtomicBool) {
    let start = Instant::now();
    while !flag.load(Ordering::Acquire) { assert!(start.elapsed() < Duration::from_secs(10)); pause(1); }
}

#[test]
fn operator_client_stops_while_a_connected_human_withholds_its_decision() {
    let root = Directory::new(); let c = configured(&root, false); let profile = profile(&root, &c, false);
    let doc = document(&c); let store = c.store.clone(); let bootstrap = c.profile.clone();
    let ready = Arc::new(AtomicBool::new(false)); let done = Arc::new(AtomicBool::new(false));
    let (human_ready, human_done, human_profile) = (Arc::clone(&ready), Arc::clone(&done), profile.clone());
    let human = std::thread::spawn(move || {
        wait(&human_profile.socket(1)); let mut client = human_profile.connect_client(1).unwrap(); let start = Instant::now();
        loop {
            assert!(start.elapsed() < Duration::from_secs(10));
            if client.step().unwrap() == ReviewClientProgress::NeedsDecision {
                human_ready.store(true, Ordering::Release); wait_flag(&human_done); return;
            }
            pause(1);
        }
    });
    let operator_profile = profile.clone();
    let operator = std::thread::spawn(move || {
        wait_flag(&ready); let mut out = Vec::new();
        let result = crate::peers::stop::request(&operator_profile, 1, &mut out);
        done.store(true, Ordering::Release);
        result.unwrap();
        let json = fa_reference::strict_json::parse(&out, fa_reference::strict_json::Limits::default()).unwrap();
        assert_eq!(json.get("status").unwrap().as_str(), Some("stopped_drained"));
    });
    let result = workflow::run_with_peers(c, &doc, false, Some(&profile), || ElapsedTick(1000)).unwrap();
    operator.join().unwrap(); human.join().unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure); assert!(!executed(&result)); assert_eq!(result.cleanup_pending, 0);
    let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert!(disk.stop.is_some()); assert_eq!(disk.executions, 0);
    assert_eq!(disk.control.ledger.charged, 0); assert_eq!(disk.control.ledger.reserved, 0);
}

#[test]
fn queued_stop_after_dispatch_uses_native_nonexecution_and_preserves_unstopped_control() {
    for stop in [false, true] {
        let root = Directory::new(); let c = configured(&root, false); let profile = profile(&root, &c, false);
        let doc = document(&c); let store = c.store.clone(); let bootstrap = c.profile.clone();
        let dispatched = Arc::new(AtomicBool::new(false)); let queued = Arc::new(AtomicBool::new(false));
        let controller = if stop {
            let (dispatched, queued, peer) = (Arc::clone(&dispatched), Arc::clone(&queued), profile.clone());
            Some(std::thread::spawn(move || {
                wait_flag(&dispatched); let mut client = client(&peer); queued.store(true, Ordering::Release);
                let start = Instant::now();
                loop {
                    assert!(start.elapsed() < Duration::from_secs(10));
                    match client.step().unwrap() {
                        StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                        StopClientProgress::Complete => { assert!(client.receipt().unwrap().drained()); return; }
                        _ => pause(1),
                    }
                }
            }))
        } else { None };
        let human = approving(profile.clone()); let mut saw_dispatch = false;
        let result = workflow::run_with_peers(c, &doc, false, Some(&profile), || {
            if !saw_dispatch && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
                && disk.control.ledger.charged != 0 && disk.executions == 0 {
                saw_dispatch = true;
                if stop { dispatched.store(true, Ordering::Release); wait_flag(&queued); }
            }
            ElapsedTick(1000)
        }).unwrap();
        human.join().unwrap(); if let Some(controller) = controller { controller.join().unwrap(); }
        assert!(saw_dispatch); assert!(result.failure.is_none(), "{:?}", result.failure);
        assert_eq!(executed(&result), !stop); assert_eq!(result.cleanup_pending, 0);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, u64::from(!stop)); assert_eq!(disk.control.ledger.reserved, 0);
        assert_eq!(disk.control.ledger.charged, if stop { 0 } else { 10 });
        assert_eq!(disk.stop.is_some(), stop);
    }
}
