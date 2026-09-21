//! The same history-loss consumer with an actual opaque-only observation.
//! No dummy key, inferred dependency subset, or helper explanation is supplied.
use super::*;
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, Omission, PartKind, SubmittedPart};

const QUERIES: &str = r#""requests":[{"kind":"exact_value","key":0,"role":"subject"},{"kind":"absent_key","key":1}]"#;
fn whole_recipe(root: &Directory, c: &Config) -> String {
    let text = snapshot_recipe(root, c);
    assert!(text.contains(QUERIES));
    text.replace("fa.supervised-witnesses/5", "fa.supervised-whole-input/2")
        .replace(QUERIES, r#""requests":[]"#)
}
fn whole_profile(root: &Directory, c: &Config) -> PublicationProfile {
    PublicationProfile::decode(whole_recipe(root, c).as_bytes()).unwrap()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View { Quiet, Bytes, Model, Layout, Omission, Missing }
fn actual(view: View) -> ActualHelperInput {
    ActualHelperInput::new(
        if view == View::Bytes { b"Q|R".to_vec() } else { b"Q|P".to_vec() },
        InputProfileBinding { profile_id: 7, profile_bytes: vec![0, 255, 1],
            tokenizer_epoch: 1, policy_epoch: 2, model_epoch: 3 + u64::from(view == View::Model) },
        vec![SubmittedPart { span: ByteSpan { start: 0, end: 1 }, kind: PartKind::Question },
            SubmittedPart { span: ByteSpan { start: 1, end: 2 }, kind: PartKind::Delimiter },
            SubmittedPart { span: ByteSpan { start: 2, end: 3 },
                kind: if view == View::Layout { PartKind::Instruction } else { PartKind::Prompt } }],
        vec![if view == View::Omission { Omission::Redacted { domain_id: 21, transform_id: 1 } }
            else { Omission::Unsupported { domain_id: 21 } }],
    ).unwrap()
}
fn opaque(view: View) -> FilePublicationInputs {
    FilePublicationInputs::new(None, if view == View::Missing { None } else { Some(actual(view)) })
}
fn whole_image(c: &Config) -> PublicationProducerImage {
    PublicationProducerImage::new(identity(c), opaque(View::Quiet), ElapsedTick(1000)).unwrap()
}
fn evict_opaque(mut image: PublicationProducerImage, final_view: View) -> PublicationProducerImage {
    let before = image.batch().heartbeat().through;
    for step in 0..258 {
        let view = if step % 2 == 0 { View::Bytes } else { View::Quiet };
        image = image.advance(image.generation(), opaque(view), ElapsedTick(1000)).unwrap();
    }
    if final_view != View::Quiet {
        image = image.advance(image.generation(), opaque(final_view), ElapsedTick(1000)).unwrap();
    }
    assert!(image.batch().after() > before);
    assert!(image.inputs().structured().is_none());
    image
}

#[test]
fn whole_snapshot_schema_is_explicit_and_never_invents_a_structured_recipe() {
    let root = Directory::new(); let c = configured(&root); let json = whole_recipe(&root, &c);
    let profile = whole_profile(&root, &c);
    assert!(profile.snapshot_fallback); assert!(profile.requests.is_empty());
    assert!(profile.wait.is_none()); assert!(matches!(profile.sources, PublicationSources::Producer { .. }));
    for invalid in [json.replace(HISTORY, ""), json.replace("exact_current_snapshot", "ignore_history"),
        json.replace("fa.supervised-whole-input/2", "fa.supervised-whole-input/1"),
        json.replace("fa.supervised-whole-input/2", "fa.supervised-witnesses/5"),
        json.replace(r#""requests":[]"#, QUERIES),
        json.replace("\"source\":91", "\"source\":91,\"max_retries\":4"),
        json.replace("\"producer\":", "\"current\":\"/fallback.bin\",\"producer\":")] {
        assert!(PublicationProfile::decode(invalid.as_bytes()).is_err(), "{invalid}");
    }
    let legacy = json.replace(HISTORY, "").replace("fa.supervised-whole-input/2", "fa.supervised-whole-input/1");
    assert!(!PublicationProfile::decode(legacy.as_bytes()).unwrap().snapshot_fallback);
    assert!(!c.store.exists()); assert!(!root.0.join("producer").exists());
}

#[test]
fn original_opaque_input_survives_eviction_and_binding_without_freshness_or_rights() {
    let root = Directory::new(); let c = configured(&root); let profile = whole_profile(&root, &c);
    let image = evict_opaque(whole_image(&c), View::Quiet); write_image(&root, &image);
    let (mut host, _) = profile.create(&c.store, c.profile.clone()).unwrap();
    let action = propose(&mut host, &c, 42);
    let prepared = profile.prepare_with_clock(&mut host, 42, || ElapsedTick(1000)).unwrap();
    let retained = host.retained_publication_evidence(42).unwrap();
    assert!(retained.requests().is_empty()); assert!(retained.original().structured().is_none());
    assert_eq!(retained.original(), image.inputs());
    assert_eq!(retained.original().opaque(), Some(&actual(View::Quiet)));
    assert_eq!(prepared.current.reader().read_capture().unwrap(), image.capture(42, &action).unwrap());
    assert!(!host.publication_source(42).unwrap().unwrap().fresh);
    assert!(!host.publication_change_status().unwrap().complete());
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.reserved, 0);
}

#[test]
fn metadata_only_or_structured_only_originals_cannot_start_whole_snapshot_review() {
    for structured in [false, true] {
        let root = Directory::new(); let mut c = configured(&root); evidence(&root, &c);
        let profile = whole_profile(&root, &c);
        let empty = if structured { inputs(&c, 1, &[0, 2, 4]) } else { FilePublicationInputs::new(None, None) };
        let image = PublicationProducerImage::new(identity(&c), empty, ElapsedTick(1000)).unwrap();
        write_image(&root, &image); let document = document(&c);
        let store = c.store.clone(); let bootstrap = c.profile.clone();
        c.programs.clear(); // Original-input admission must fail before roster launch.
        let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || ElapsedTick(1000)).unwrap();
        assert!(result.failure.as_ref().unwrap().contains("Incomplete"), "{:?}", result.failure);
        assert!(!executed(&result)); assert_eq!(result.cleanup_pending, 0);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, 0); assert_eq!(disk.control.ledger.reserved, 0); assert_eq!(disk.control.ledger.charged, 0);
    }
}

#[test]
fn opaque_snapshot_publication_and_exact_retry_need_no_dummy_keys_or_live_recovery_sources() {
    let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
    let profile = whole_profile(&root, &c);
    write_image(&root, &evict_opaque(whole_image(&c), View::Quiet));
    let document = document(&c); let human = reviewer(&c);
    let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || ElapsedTick(1000)).unwrap();
    human.join().unwrap(); assert!(executed(&result)); assert!(result.failure.is_none());
    std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let c = configured(&root); let before = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    let legacy = whole_recipe(&root, &c).replace(HISTORY, "")
        .replace("fa.supervised-whole-input/2", "fa.supervised-whole-input/1");
    let strict = PublicationProfile::decode(legacy.as_bytes()).unwrap();
    assert!(strict.open(&c.store, c.profile.clone()).is_err());
    assert_eq!(FileOversight::read_publication(&c.store, &c.profile).unwrap(), before);
    let mut c = configured(&root); c.programs.clear();
    let recovered = workflow::run_with_publication(c, &document, true, None, Some(&profile), || ElapsedTick(200000)).unwrap();
    assert!(executed(&recovered)); assert!(recovered.failure.is_none());
    let mut c = configured(&root); c.programs.clear();
    let retry = workflow::continuation::submit_existing(c, &document, None, Some(&profile), || ElapsedTick(200001)).unwrap();
    assert!(executed(&retry)); assert!(retry.failure.is_none()); assert_eq!(retry.cleanup_pending, 0);
    let c = configured(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.control.ledger.charged, 7); assert_eq!(disk.control.ledger.reserved, 0);
}

#[test]
fn lost_history_never_rebases_changed_opaque_bytes_layout_model_or_omissions() {
    for view in [View::Quiet, View::Bytes, View::Model, View::Layout, View::Omission, View::Missing] {
        let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
        let profile = whole_profile(&root, &c); let image = whole_image(&c); write_image(&root, &image);
        let next = evict_opaque(image, view);
        let store = c.store.clone(); let bootstrap = c.profile.clone(); let mut changed = false;
        let document = document(&c); let human = reviewer(&c);
        let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || {
            if !changed && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
                && disk.control.ledger.charged != 0 && disk.executions == 0 {
                write_image(&root, &next); changed = true;
            }
            ElapsedTick(1000)
        }).unwrap();
        human.join().unwrap(); assert!(changed, "{view:?}"); assert!(result.failure.is_none(), "{view:?}: {:?}", result.failure);
        let allowed = view == View::Quiet;
        assert_eq!(executed(&result), allowed, "{view:?}"); assert_eq!(result.cleanup_pending, 0);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, u64::from(allowed)); assert_eq!(disk.control.ledger.reserved, 0);
        assert_eq!(disk.control.ledger.charged, if allowed { 7 } else { 0 });
    }
}

#[test]
fn full_view_comparison_budget_is_required_even_when_the_current_bytes_match() {
    for exhausted in [false, true] {
        let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
        let mut json = whole_recipe(&root, &c);
        if exhausted { json = json.replace(r#""limits":{"bindings":8,"steps":10000"#, r#""limits":{"bindings":8,"steps":0"#); }
        let profile = PublicationProfile::decode(json.as_bytes()).unwrap();
        assert_eq!(profile.limits.validation.steps == 0, exhausted);
        write_image(&root, &evict_opaque(whole_image(&c), View::Quiet));
        let store = c.store.clone(); let bootstrap = c.profile.clone();
        let document = document(&c); let human = reviewer(&c);
        let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || ElapsedTick(1000)).unwrap();
        human.join().unwrap(); assert_eq!(executed(&result), !exhausted);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, u64::from(!exhausted)); assert_eq!(disk.control.ledger.reserved, 0);
        assert_eq!(disk.control.ledger.charged, if exhausted { 0 } else { 7 });
    }
}
