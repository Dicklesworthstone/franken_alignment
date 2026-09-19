//! Full runnable workflow with the actual native feed reader and leased source.
//! No test turns a synthetic helper verdict or heartbeat into an effect permit.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::PublicationFeedBatch;
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChange;
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::PublicationHeartbeat;
use fa_reference::witness::refinement::index::routing::WitnessChange;

fn profile_json(root: &Directory) -> String {
    let original = super::profile_json(root).replace("fa.supervised-witnesses/1", "fa.supervised-witnesses/2");
    let feed = format!(r#""feed":{{"source":41,"path":"{}/feed.bin","after":0,"clock":"unix_milliseconds","max_age_ms":20,"lookup":{{"steps":10000,"bytes":1048576}}}},"#,
        root.0.display());
    original.replacen('{', &format!("{{{feed}"), 1)
}
fn feed_profile(root: &Directory) -> PublicationProfile {
    PublicationProfile::decode(profile_json(root).as_bytes()).unwrap()
}
fn batch(action: &FrozenAction, generation: u64, after: u64, keys: &[u64], produced_at: u64) -> PublicationFeedBatch {
    let domain = DomainProjection::new(40, 1, ProjectionKey {
        source: 40, branch: action.spec().scope.branch, projection: 7, source_epoch: 1,
    });
    PublicationFeedBatch::new(PublicationHeartbeat { source: 41, clock_domain: CLOCK_DOMAIN,
        generation, through: after + keys.len() as u64, produced_at: ElapsedTick(produced_at) }, after,
        keys.iter().enumerate().map(|(i, key)| PublicationChange { source: 41,
            sequence: after + i as u64 + 1, change: WitnessChange::Key { domain, key: *key } }).collect()).unwrap()
}
fn write_feed(root: &Directory, batch: &PublicationFeedBatch) {
    let pending = root.0.join("feed.next");
    std::fs::write(&pending, batch.to_bytes().unwrap()).unwrap(); std::fs::rename(pending, root.0.join("feed.bin")).unwrap();
}

#[test]
fn feed_profile_is_explicit_and_all_three_policies_enter_the_first_canonical_image() {
    let root = Directory::new(); let json = profile_json(&root); let selected = feed_profile(&root);
    for invalid in [json.replace("\"source\":41", "\"source\":0"),
        json.replace("\"max_age_ms\":20", "\"max_age_ms\":0"),
        json.replace("unix_milliseconds", "process_instant"),
        json.replace("fa.supervised-witnesses/2", "fa.supervised-witnesses/1"),
        super::profile_json(&root).replace("fa.supervised-witnesses/1", "fa.supervised-witnesses/2"),
        json.replace("\"after\":0", "\"after\":-1"), json.replace("\"after\":0", "\"after\":0,\"after\":1"),
        json.replace("\"lookup\":{\"steps\":10000,\"bytes\":1048576}", "\"lookup\":null"),
        json.replace("\"source\":41", "\"source\":41,\"permissive\":true")] {
        assert!(PublicationProfile::decode(invalid.as_bytes()).is_err());
    }
    let c = configured(&root);
    let (host, _) = selected.create(&c.store, c.profile.clone()).unwrap();
    assert_eq!(host.revision(), 3);
    assert_eq!(host.publication_validation_profile().unwrap(), Some(selected.limits));
    assert_eq!(host.publication_change_status().unwrap().source, 41);
    assert_eq!(host.publication_change_status().unwrap().through, 0);
    assert_eq!(FileOversight::read_publication(&c.store, &c.profile).unwrap(), host.inspect());
    let bad_root = Directory::new(); let mut bad = configured(&bad_root); bad.profile.delivery.clock_domain += 1;
    assert!(selected.create(&bad.store, bad.profile).is_err()); assert!(!bad.store.exists());
}

#[test]
fn actual_feed_catchup_publishes_and_original_requirements_remain_bound() {
    let root = Directory::new(); let c = configured(&root); let payload = b"live producer";
    let (_, action) = prepare(&root, &c, payload); let selected = feed_profile(&root);
    write_feed(&root, &batch(&action, 1, 0, &[99, 100], 1000));
    let document = proposal(&c, payload); let human = reviewer(&c);
    let result = workflow::run_with_publication(c, &document, false, None, Some(&selected), || ElapsedTick(1000)).unwrap();
    human.join().unwrap(); assert!(result.failure.is_none(), "{:?}", result.failure); assert!(executed(&result));
    assert_eq!(result.cleanup_pending, 0);
    let c = configured(&root); let (host, _) = selected.open(&c.store, c.profile).unwrap();
    assert_eq!(host.publication_change_status().unwrap().through, 2);
    assert_eq!(host.retained_publication_evidence(1).unwrap().requests().len(), 4);
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().payload, payload);
    assert_eq!(host.inspect().control.ledger.charged, payload.len() as u64);
}

#[test]
fn missing_gapped_and_expired_feeds_refuse_even_when_both_keys_and_witnesses_are_valid() {
    for mode in 0..4 {
        let root = Directory::new(); let c = configured(&root); let payload = b"no freshness fallback";
        let (_, action) = prepare(&root, &c, payload); let selected = feed_profile(&root);
        match mode {
            0 => {}, // no feed, not an empty complete feed
            1 => write_feed(&root, &batch(&action, 1, 1, &[99], 1000)), // missing sequence 1
            2 => write_feed(&root, &batch(&action, 1, 0, &[], 980)), // exact expiry
            _ => write_feed(&root, &batch(&action, 1, 0, &[], 981)), // near-identical live control
        }
        let document = proposal(&c, payload); let human = reviewer(&c);
        let result = workflow::run_with_publication(c, &document, false, None, Some(&selected), || ElapsedTick(1000)).unwrap();
        human.join().unwrap(); assert_eq!(executed(&result), mode == 3);
        let c = configured(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
        assert_eq!(disk.executions, u64::from(mode == 3)); assert_eq!(disk.control.ledger.reserved, 0);
        assert_eq!(disk.control.ledger.charged, if mode == 3 { payload.len() as u64 } else { 0 });
    }
}

#[test]
fn post_dispatch_feed_notifications_preserve_phantom_and_unrelated_change_distinction() {
    for phantom in [false, true] {
        let root = Directory::new(); let c = configured(&root); let payload = b"feed plus original reads";
        let (_, action) = prepare(&root, &c, payload); let selected = feed_profile(&root);
        write_feed(&root, &batch(&action, 1, 0, &[99], 1000));
        let document = proposal(&c, payload); let human = reviewer(&c);
        let store = c.store.clone(); let bootstrap = c.profile.clone(); let mut changed = false;
        let result = workflow::run_with_publication(c, &document, false, None, Some(&selected), || {
            if !changed && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
                && disk.control.ledger.charged != 0 && disk.executions == 0 {
                let keys: &[u64] = if phantom { &[0, 1, 2, 4] } else { &[0, 2, 4, 100] };
                write(&root.0.join("current.bin"), &packet(&action, 2, keys));
                write_feed(&root, &batch(&action, 2, 1, &[if phantom { 1 } else { 100 }], 1000));
                changed = true;
            }
            ElapsedTick(1000)
        }).unwrap();
        human.join().unwrap(); assert!(changed); assert_eq!(executed(&result), !phantom);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, u64::from(!phantom));
        assert_eq!(disk.control.ledger.charged, if phantom { 0 } else { payload.len() as u64 });
    }
}

#[test]
fn post_dispatch_feed_loss_seals_but_conflicting_history_remains_an_unresolved_charge() {
    for conflict in [false, true] {
        let root = Directory::new(); let c = configured(&root); let payload = b"do not resend";
        let (_, action) = prepare(&root, &c, payload); let selected = feed_profile(&root);
        write_feed(&root, &batch(&action, 1, 0, &[99], 1000));
        let document = proposal(&c, payload); let human = reviewer(&c);
        let store = c.store.clone(); let bootstrap = c.profile.clone(); let mut changed = false;
        let result = workflow::run_with_publication(c, &document, false, None, Some(&selected), || {
            if !changed && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
                && disk.control.ledger.charged != 0 && disk.executions == 0 {
                if conflict { write_feed(&root, &batch(&action, 2, 0, &[100], 1000)); }
                else { std::fs::remove_file(root.0.join("feed.bin")).unwrap(); }
                changed = true;
            }
            ElapsedTick(1000)
        }).unwrap();
        human.join().unwrap(); assert!(changed); assert!(!executed(&result));
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, 0); assert_eq!(disk.payload, b"initial");
        // Source installation ambiguity is not a native nonexecution receipt.
        assert_eq!(disk.control.ledger.charged, if conflict { payload.len() as u64 } else { 0 });
        if conflict { assert!(result.failure.is_some()); }
    }
}

#[test]
fn fresh_feed_does_not_replace_the_independent_native_policy_source() {
    let root = Directory::new(); let c = configured(&root); let payload = b"two source obligations";
    let (_, action) = prepare(&root, &c, payload); let selected = feed_profile(&root);
    write_feed(&root, &batch(&action, 1, 0, &[], 1000));
    let document = proposal(&c, payload); let human = reviewer(&c);
    let store = c.store.clone(); let bootstrap = c.profile.clone(); let mut removed = false;
    let result = workflow::run_with_publication(c, &document, false, None, Some(&selected), || {
        if !removed && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
            && disk.control.ledger.charged != 0 && disk.executions == 0 {
            std::fs::remove_file(root.0.join("evidence.json")).unwrap(); removed = true;
        }
        ElapsedTick(1000)
    }).unwrap();
    human.join().unwrap(); assert!(removed); assert!(!executed(&result));
    assert_eq!(FileOversight::read_publication(&store, &bootstrap).unwrap().executions, 0);
}

#[test]
fn feed_recovery_needs_no_producer_and_pins_the_entire_required_profile() {
    let root = Directory::new(); let c = configured(&root); let payload = b"historical outcome";
    let (_, action) = prepare(&root, &c, payload); let selected = feed_profile(&root);
    write_feed(&root, &batch(&action, 1, 0, &[], 1000));
    let document = proposal(&c, payload); let human = reviewer(&c);
    assert!(executed(&workflow::run_with_publication(c, &document, false, None, Some(&selected), || ElapsedTick(1000)).unwrap()));
    human.join().unwrap();
    for name in ["original.bin", "current.bin", "feed.bin", "evidence.json"] { std::fs::remove_file(root.0.join(name)).unwrap(); }
    let mut c = configured(&root); c.programs.clear();
    assert!(executed(&workflow::run_with_publication(c, &document, true, None, Some(&selected), || ElapsedTick(1001)).unwrap()));
    let c = configured(&root); let before = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    for invalid in [profile_json(&root).replace("\"source\":41", "\"source\":42"),
        profile_json(&root).replace("\"max_age_ms\":20", "\"max_age_ms\":21"),
        profile_json(&root).replace("\"after\":0", "\"after\":1"),
        profile_json(&root).replace("\"bytes\":1048576", "\"bytes\":1048577")] {
        let wrong = PublicationProfile::decode(invalid.as_bytes()).unwrap();
        assert!(wrong.open(&c.store, c.profile.clone()).is_err());
        assert_eq!(FileOversight::read_publication(&c.store, &c.profile).unwrap(), before);
    }
    let weaker = PublicationProfile::decode(super::profile_json(&root).as_bytes()).unwrap();
    assert!(weaker.open(&c.store, c.profile).is_err()); // original fence may advance, but no weakened owner escapes
}
