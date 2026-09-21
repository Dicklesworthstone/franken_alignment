//! Runnable exact-snapshot selection over actual producer, helper and reviewer
//! boundaries. Synthetic helper answers exercise enforcement, not model quality.
use super::*;
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::PublicationProducerImage;

const HISTORY: &str = "\"history\":\"exact_current_snapshot\",";

fn snapshot_recipe(root: &Directory, c: &Config) -> String {
    recipe(root, c).replace("fa.supervised-witnesses/3", "fa.supervised-witnesses/5")
        .replacen('{', &format!("{{{HISTORY}"), 1)
}
fn selected(root: &Directory, c: &Config) -> PublicationProfile {
    PublicationProfile::decode(snapshot_recipe(root, c).as_bytes()).unwrap()
}
fn original(c: &Config) -> PublicationProducerImage {
    PublicationProducerImage::new(identity(c), inputs(c, 1, &[0, 2, 4]), ElapsedTick(1000)).unwrap()
}
fn evict(c: &Config, mut image: PublicationProducerImage, keys: &[u64]) -> PublicationProducerImage {
    let previous_head = image.batch().heartbeat().through;
    let mut revision = image.inputs().structured().unwrap().snapshot().revision();
    // The REAL producer derives inserts/deletes. No fabricated covered sequence
    // or manually truncated feed is supplied to make this fixture pass.
    for _ in 0..130 {
        revision += 1;
        let mut current = keys.to_vec();
        current.push(if revision % 2 == 0 { 99 } else { 100 });
        image = image.advance(image.generation(), inputs(c, revision, &current), ElapsedTick(1000)).unwrap();
    }
    assert!(image.batch().after() > previous_head, "the earlier head really left retention");
    image
}
fn write_image(root: &Directory, image: &PublicationProducerImage) {
    std::fs::create_dir_all(root.0.join("producer")).unwrap();
    let pending = root.0.join("producer/next.bin");
    std::fs::write(&pending, image.to_bytes().unwrap()).unwrap();
    std::fs::rename(pending, root.0.join("producer/delivery.bin")).unwrap();
}

#[test]
fn snapshot_schema_requires_explicit_history_and_preserves_legacy_admission() {
    let root = Directory::new(); let c = configured(&root);
    let valid = snapshot_recipe(&root, &c);
    let profile = PublicationProfile::decode(valid.as_bytes()).unwrap();
    assert!(profile.snapshot_fallback); assert!(profile.wait.is_none());
    assert!(profile.feed.is_some()); assert_eq!(profile.requests.len(), 2);
    assert!(!PublicationProfile::decode(recipe(&root, &c).as_bytes()).unwrap().snapshot_fallback);
    let requests = r#"[{"kind":"exact_value","key":0,"role":"subject"},{"kind":"absent_key","key":1}]"#;
    for invalid in [valid.replace(HISTORY, ""),
        valid.replace("exact_current_snapshot", "assume_complete"),
        valid.replace("\"exact_current_snapshot\"", "true"),
        valid.replace(HISTORY, &format!("{HISTORY}{HISTORY}")),
        valid.replace("fa.supervised-witnesses/5", "fa.supervised-witnesses/3"),
        valid.replace("fa.supervised-witnesses/5", "fa.supervised-witnesses/4"),
        valid.replace("fa.supervised-witnesses/5", "fa.supervised-whole-input/1"),
        valid.replace(requests, "[]"),
        valid.replace("\"source\":91", "\"source\":91,\"max_retries\":1"),
        valid.replace("\"producer\":", "\"original\":\"/cached.bin\",\"producer\":"),
        valid.replace("\"after\":0", "\"after\":0,\"path\":\"/split-feed.bin\""),
        valid.replace("\"max_age_ms\":5000", "\"max_age_ms\":0"),
        valid.replace("\"source\":41", "\"source\":0")] {
        assert!(PublicationProfile::decode(invalid.as_bytes()).is_err(), "{invalid}");
    }
    assert!(!c.store.exists()); assert!(!root.0.join("producer").exists());
}

#[test]
fn history_selection_is_in_the_first_image_and_pinned_before_recovery_writes() {
    for fallback in [false, true] {
        let root = Directory::new(); let c = configured(&root);
        let strict = PublicationProfile::decode(recipe(&root, &c).as_bytes()).unwrap();
        let exact = selected(&root, &c);
        let (chosen, other) = if fallback { (&exact, &strict) } else { (&strict, &exact) };
        let (mut host, reviewer) = chosen.create(&c.store, c.profile.clone()).unwrap();
        assert_eq!(host.revision(), if fallback { 4 } else { 3 });
        assert_eq!(host.publication_snapshot_fallback_enabled().unwrap(), fallback);
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
        let before = host.inspect(); drop(reviewer); drop(host);
        assert!(matches!(other.open(&c.store, c.profile.clone()), Err(JournalError::Contract(Error::Binding))));
        assert_eq!(FileOversight::read_publication(&c.store, &c.profile).unwrap(), before);
        let (host, _) = chosen.open(&c.store, c.profile.clone()).unwrap();
        assert_eq!(host.publication_snapshot_fallback_enabled().unwrap(), fallback);
        assert!(!host.clock_ready());
    }
    let root = Directory::new(); let c = configured(&root); let profile = selected(&root, &c);
    let mut wrong = c.profile.clone(); wrong.delivery.clock_domain += 1;
    assert!(profile.create(&c.store, wrong).is_err()); assert!(!c.store.exists());
}

#[test]
fn both_preparation_apis_reject_a_different_history_policy_before_io_or_clock() {
    for fallback in [false, true] {
        let root = Directory::new(); let c = configured(&root);
        let strict = PublicationProfile::decode(recipe(&root, &c).as_bytes()).unwrap();
        let exact = selected(&root, &c);
        let (chosen, other) = if fallback { (&exact, &strict) } else { (&strict, &exact) };
        let (mut host, _) = chosen.create(&c.store, c.profile.clone()).unwrap();
        propose(&mut host, &c, 42); let before = host.inspect();
        let error = other.prepare_with_clock(&mut host, 42, || panic!("wrong profile must not acquire time"))
            .err().unwrap();
        assert_eq!(error, "stored publication history policy differs from the explicit profile");
        assert_eq!(other.prepare(&mut host, 42).err().unwrap(), error);
        assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_none());
        assert!(matches!(host.retained_publication_evidence(42), Err(JournalError::Contract(Error::Missing))));
        assert!(!root.0.join("producer").exists());
    }
}

#[test]
fn evicted_original_can_bind_without_manufacturing_history_or_current_eligibility() {
    for fallback in [false, true] {
        let root = Directory::new(); let c = configured(&root);
        let profile = if fallback { selected(&root, &c) }
            else { PublicationProfile::decode(recipe(&root, &c).as_bytes()).unwrap() };
        let image = evict(&c, original(&c), &[0, 2, 4]); write_image(&root, &image);
        let (mut host, _) = profile.create(&c.store, c.profile.clone()).unwrap();
        let action = propose(&mut host, &c, 42);
        let result = profile.prepare_with_clock(&mut host, 42, || ElapsedTick(1000));
        if !fallback {
            assert!(result.is_err()); assert!(host.storage_failure().is_some());
            assert!(host.retained_publication_evidence(42).is_err());
            continue;
        }
        let prepared = result.unwrap();
        assert_eq!(prepared.current.reader().read_capture().unwrap(), image.capture(42, &action).unwrap());
        let status = host.publication_change_status().unwrap();
        assert_eq!(status.through, 0); assert_eq!(status.observed_through, image.batch().heartbeat().through);
        assert!(!status.complete());
        assert_eq!(host.publication_change_freshness().unwrap().eligibility, Err(Error::Incomplete));
        let binding = host.retained_publication_evidence(42).unwrap();
        assert_eq!(binding.original(), image.inputs()); assert_eq!(binding.requests(), profile.requests.as_slice());
        assert!(!host.publication_source(42).unwrap().unwrap().fresh);
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.reserved, 0);
    }
}

#[test]
fn runnable_snapshot_mode_publishes_after_retention_loss_and_recovers_without_sources() {
    let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
    let profile = selected(&root, &c); let image = evict(&c, original(&c), &[0, 2, 4]);
    write_image(&root, &image); let document = document(&c); let human = reviewer(&c);
    let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || ElapsedTick(1000)).unwrap();
    human.join().unwrap(); assert!(result.failure.is_none(), "{:?}", result.failure);
    assert!(executed(&result)); assert_eq!(result.cleanup_pending, 0);
    let c = configured(&root);
    let before = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(before.executions, 1); assert_eq!(before.control.ledger.charged, 7);
    std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let strict = PublicationProfile::decode(recipe(&root, &c).as_bytes()).unwrap();
    assert!(strict.open(&c.store, c.profile.clone()).is_err());
    assert_eq!(FileOversight::read_publication(&c.store, &c.profile).unwrap(), before);
    let mut c = configured(&root); c.programs.clear();
    let recovered = workflow::run_with_publication(c, &document, true, None, Some(&profile), || ElapsedTick(200000)).unwrap();
    assert!(executed(&recovered)); assert!(recovered.failure.is_none()); assert_eq!(recovered.cleanup_pending, 0);
    let c = configured(&root); let (host, _) = profile.open(&c.store, c.profile.clone()).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 7);
    assert!(!host.publication_change_status().unwrap().complete());
}

#[test]
fn post_dispatch_loss_of_notifications_never_rebases_the_original_negative_witness() {
    for phantom in [false, true] {
        let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
        let profile = selected(&root, &c); let image = original(&c); write_image(&root, &image);
        let next = evict(&c, image, if phantom { &[0, 1, 2, 4] } else { &[0, 2, 4] });
        let store = c.store.clone(); let bootstrap = c.profile.clone(); let mut changed = false;
        let document = document(&c); let human = reviewer(&c);
        let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || {
            if !changed && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
                && disk.control.ledger.charged != 0 && disk.executions == 0 {
                write_image(&root, &next); changed = true;
            }
            ElapsedTick(1000)
        }).unwrap();
        human.join().unwrap(); assert!(changed); assert_eq!(executed(&result), !phantom);
        assert!(result.failure.is_none(), "{:?}", result.failure);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, u64::from(!phantom)); assert_eq!(disk.control.ledger.reserved, 0);
        assert_eq!(disk.control.ledger.charged, if phantom { 0 } else { 7 });
    }
}

#[test]
fn new_schema_does_not_extend_the_producer_lease_at_its_expiry_boundary() {
    for expired in [false, true] {
        let root = Directory::new(); let c = configured(&root);
        let profile = selected(&root, &c); let image = evict(&c, original(&c), &[0, 2, 4]);
        write_image(&root, &image); let (mut host, _) = profile.create(&c.store, c.profile.clone()).unwrap();
        propose(&mut host, &c, 42);
        let now = if expired { 6000 } else { 5999 };
        let result = profile.prepare_with_clock(&mut host, 42, || ElapsedTick(now));
        assert_eq!(result.is_ok(), !expired);
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.reserved, 0);
        if !expired {
            assert_eq!(host.publication_change_freshness().unwrap().heartbeat.unwrap().produced_at, ElapsedTick(1000));
        }
    }
}

#[test]
fn native_policy_source_loss_still_seals_a_fresh_exact_snapshot_publication() {
    let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
    let profile = selected(&root, &c); write_image(&root, &evict(&c, original(&c), &[0, 2, 4]));
    let store = c.store.clone(); let bootstrap = c.profile.clone(); let mut removed = false;
    let document = document(&c); let human = reviewer(&c);
    let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || {
        if !removed && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
            && disk.control.ledger.charged != 0 && disk.executions == 0 {
            std::fs::remove_file(root.0.join("evidence.json")).unwrap(); removed = true;
        }
        ElapsedTick(1000)
    }).unwrap();
    human.join().unwrap(); assert!(removed); assert!(!executed(&result));
    let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(disk.executions, 0); assert_eq!(disk.control.ledger.charged, 0);
}
