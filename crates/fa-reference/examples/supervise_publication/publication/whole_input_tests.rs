//! Full-view publication through the same actor, helper, reviewer and driver.
//! Synthetic verdicts exercise enforcement, not provenance or detector accuracy.
use super::*;
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, Omission, PartKind, SubmittedPart};

const KEY_RECIPE: &str = r#""requests":[{"kind":"exact_value","key":0,"role":"subject"},{"kind":"absent_key","key":1}]"#;
fn whole_recipe(root: &Directory, c: &Config) -> String {
    let text = recipe(root, c);
    assert!(text.contains(KEY_RECIPE));
    text.replace("fa.supervised-witnesses/3", "fa.supervised-whole-input/1")
        .replace(KEY_RECIPE, r#""requests":[]"#)
}
fn whole_profile(root: &Directory, c: &Config) -> PublicationProfile {
    PublicationProfile::decode(whole_recipe(root, c).as_bytes()).unwrap()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change { Quiet, Bytes, Profile, Tokenizer, Policy, Model, Layout, Omission, Missing, ExtraLane }
fn actual(change: Change) -> ActualHelperInput {
    ActualHelperInput::new(
        if change == Change::Bytes { b"Q|R".to_vec() } else { b"Q|P".to_vec() },
        InputProfileBinding { profile_id: 7,
            profile_bytes: if change == Change::Profile { b"b".to_vec() } else { b"a".to_vec() },
            tokenizer_epoch: 1 + u64::from(change == Change::Tokenizer),
            policy_epoch: 2 + u64::from(change == Change::Policy),
            model_epoch: 3 + u64::from(change == Change::Model) },
        vec![SubmittedPart { span: ByteSpan { start: 0, end: 1 }, kind: PartKind::Question },
            SubmittedPart { span: ByteSpan { start: 1, end: 2 }, kind: PartKind::Delimiter },
            SubmittedPart { span: ByteSpan { start: 2, end: 3 },
                kind: if change == Change::Layout { PartKind::Instruction } else { PartKind::Prompt } }],
        vec![if change == Change::Omission { Omission::Redacted { domain_id: 21, transform_id: 1 } }
            else { Omission::Unsupported { domain_id: 21 } }],
    ).unwrap()
}
fn opaque_inputs(change: Change) -> FilePublicationInputs {
    FilePublicationInputs::new(None, if change == Change::Missing { None } else { Some(actual(change)) })
}

#[test]
fn explicit_whole_input_profile_preserves_all_legacy_recipe_requirements() {
    let root = Directory::new(); let c = configured(&root);
    let profile = whole_profile(&root, &c);
    assert!(profile.requests.is_empty()); assert!(profile.feed.is_some());
    let structured = recipe(&root, &c);
    assert!(PublicationProfile::decode(structured.replace(KEY_RECIPE, r#""requests":[]"#).as_bytes()).is_err());
    assert!(PublicationProfile::decode(structured.replace("fa.supervised-witnesses/3", "fa.supervised-whole-input/1").as_bytes()).is_err());
    let legacy = r#"{"schema":"fa.supervised-witnesses/1","source":91,"original":"/original.bin","current":"/current.bin","limits":{"bindings":8,"steps":10000,"value_bytes":1048576},"requests":[{"kind":"absent_key","key":1}]}"#;
    assert!(PublicationProfile::decode(legacy.as_bytes()).is_ok());
    let empty = legacy.replace(r#"[{"kind":"absent_key","key":1}]"#, "[]");
    assert!(PublicationProfile::decode(empty.as_bytes()).is_err());
    let feed = r#", "feed":{"source":41,"path":"/feed.bin","after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{"steps":10000,"bytes":1048576}}"#;
    let legacy2 = legacy.replace("/1\"", "/2\"");
    let legacy2 = format!("{}{feed}}}", legacy2.strip_suffix('}').unwrap());
    assert!(PublicationProfile::decode(legacy2.as_bytes()).is_ok());
    assert!(PublicationProfile::decode(legacy2.replace(r#"[{"kind":"absent_key","key":1}]"#, "[]").as_bytes()).is_err());
    assert!(!c.store.exists()); assert!(!root.0.join("producer").exists());
}

#[test]
fn whole_input_binding_preserves_every_original_lane_and_metadata_record() {
    for with_structured in [false, true] {
        let root = Directory::new(); let c = configured(&root); let profile = whole_profile(&root, &c);
        let structured = inputs(&c, 1, &[0]);
        let original = FilePublicationInputs::new(if with_structured { structured.structured().cloned() } else { None }, Some(actual(Change::Quiet)));
        let (mut producer, _) = FilePublicationProducer::create(root.0.join("producer"), identity(&c), original.clone(), ElapsedTick(1000)).unwrap();
        let (mut host, _) = profile.create(&c.store, c.profile.clone()).unwrap();
        let action = propose(&mut host, &c, 42); let prepared = profile.prepare(&mut host, 42).unwrap();
        let retained = host.retained_publication_evidence(42).unwrap();
        assert!(retained.requests().is_empty()); assert_eq!(retained.original(), &original);
        assert_eq!(prepared.current.reader().read_capture().unwrap(), producer.image().capture(42, &action).unwrap());
        producer.publish(1, original.clone(), ElapsedTick(1001)).unwrap();
        assert_eq!(producer.image().generation(), 2); assert_eq!(producer.image().input_generation(), 1);
        assert_eq!(host.retained_publication_evidence(42).unwrap().original(), &original);
    }
}

#[test]
fn empty_recipe_without_original_opaque_input_refuses_before_helper_launch() {
    for with_structured in [false, true] {
        let root = Directory::new(); let mut c = configured(&root); evidence(&root, &c);
        let profile = whole_profile(&root, &c);
        let original = if with_structured { inputs(&c, 1, &[0]) } else { FilePublicationInputs::new(None, None) };
        let (_producer, _) = FilePublicationProducer::create(root.0.join("producer"), identity(&c), original, ElapsedTick(1000)).unwrap();
        let document = document(&c); let store = c.store.clone(); let bootstrap = c.profile.clone();
        c.programs.clear(); // The earlier missing-original failure must win over roster validation.
        let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || ElapsedTick(1000)).unwrap();
        assert!(result.failure.as_ref().unwrap().contains("whole-input"), "{:?}", result.failure);
        assert!(!executed(&result)); assert_eq!(result.cleanup_pending, 0);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, 0); assert_eq!(disk.control.ledger.charged, 0); assert_eq!(disk.control.ledger.reserved, 0);
    }
}

#[test]
fn nonempty_structured_recipe_never_falls_back_to_an_opaque_only_image() {
    let root = Directory::new(); let c = configured(&root);
    let profile = PublicationProfile::decode(recipe(&root, &c).as_bytes()).unwrap();
    let (_producer, _) = FilePublicationProducer::create(root.0.join("producer"), identity(&c), opaque_inputs(Change::Quiet), ElapsedTick(1000)).unwrap();
    let (mut host, _) = profile.create(&c.store, c.profile.clone()).unwrap();
    propose(&mut host, &c, 42); let revision = host.revision();
    let error = profile.prepare(&mut host, 42).err().unwrap();
    assert!(error.contains("structured")); assert_eq!(host.revision(), revision);
}

#[test]
fn every_whole_input_dependency_is_rechecked_after_dispatch() {
    for change in [Change::Quiet, Change::Bytes, Change::Profile, Change::Tokenizer, Change::Policy,
        Change::Model, Change::Layout, Change::Omission, Change::Missing, Change::ExtraLane] {
        let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
        let profile = whole_profile(&root, &c);
        let (mut producer, _) = FilePublicationProducer::create(root.0.join("producer"), identity(&c), opaque_inputs(Change::Quiet), ElapsedTick(1000)).unwrap();
        let changed_inputs = if change == Change::ExtraLane {
            FilePublicationInputs::new(inputs(&c, 1, &[0]).structured().cloned(), Some(actual(Change::Quiet)))
        } else { opaque_inputs(change) };
        let document = document(&c); let store = c.store.clone(); let bootstrap = c.profile.clone();
        let human = reviewer(&c); let mut changed = false;
        let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || {
            if !changed && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
                && disk.control.ledger.charged != 0 && disk.executions == 0 {
                producer.publish(1, changed_inputs.clone(), ElapsedTick(1000)).unwrap(); changed = true;
            }
            ElapsedTick(1000)
        }).unwrap();
        human.join().unwrap(); assert!(changed, "{change:?}"); assert_eq!(result.cleanup_pending, 0);
        let allowed = change == Change::Quiet;
        assert_eq!(executed(&result), allowed, "{change:?}: {:?}", result.failure);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, u64::from(allowed), "{change:?}");
        assert_eq!(disk.control.ledger.reserved, 0); assert_eq!(disk.control.ledger.charged, if allowed { 7 } else { 0 });
        assert_eq!(disk.payload.as_slice(), if allowed { b"publish".as_slice() } else { b"initial".as_slice() });
    }
}

#[test]
fn whole_input_mode_keeps_native_comparison_budget_and_freshness_required() {
    for case in ["valid", "budget", "expired"] {
        let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
        let mut recipe = whole_recipe(&root, &c);
        if case == "budget" {
            recipe = recipe.replace(r#""limits":{"bindings":8,"steps":10000"#, r#""limits":{"bindings":8,"steps":0"#);
        }
        if case == "expired" { recipe = recipe.replace(r#""max_age_ms":5000"#, r#""max_age_ms":1"#); }
        let profile = PublicationProfile::decode(recipe.as_bytes()).unwrap();
        let (_producer, _) = FilePublicationProducer::create(root.0.join("producer"), identity(&c), opaque_inputs(Change::Quiet),
            ElapsedTick(if case == "expired" { 0 } else { 1000 })).unwrap();
        let document = document(&c); let human = reviewer(&c); let store = c.store.clone(); let bootstrap = c.profile.clone();
        let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || ElapsedTick(1000)).unwrap();
        human.join().unwrap(); assert_eq!(executed(&result), case == "valid", "{case}: {:?}", result.failure);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, u64::from(case == "valid")); assert_eq!(disk.control.ledger.reserved, 0);
        assert_eq!(disk.control.ledger.charged, if case == "valid" { 7 } else { 0 });
    }
}

#[test]
fn whole_input_receipts_remain_query_only_without_any_live_source() {
    let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
    let profile = whole_profile(&root, &c);
    let (producer, _) = FilePublicationProducer::create(root.0.join("producer"), identity(&c), opaque_inputs(Change::Quiet), ElapsedTick(1000)).unwrap();
    let document = document(&c); let human = reviewer(&c);
    let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || ElapsedTick(1000)).unwrap();
    human.join().unwrap(); assert!(executed(&result)); assert!(result.failure.is_none());
    drop(producer); std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
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
fn unrelated_structured_updates_do_not_replace_or_narrow_the_full_view() {
    for change_opaque in [false, true] {
        let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
        let profile = whole_profile(&root, &c);
        let original = FilePublicationInputs::new(inputs(&c, 1, &[0]).structured().cloned(), Some(actual(Change::Quiet)));
        let (mut producer, _) = FilePublicationProducer::create(root.0.join("producer"), identity(&c), original, ElapsedTick(1000)).unwrap();
        let changed_inputs = FilePublicationInputs::new(inputs(&c, 2, &[0, 99]).structured().cloned(),
            Some(actual(if change_opaque { Change::Model } else { Change::Quiet })));
        let document = document(&c); let store = c.store.clone(); let bootstrap = c.profile.clone();
        let human = reviewer(&c); let mut changed = false;
        let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || {
            if !changed && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
                && disk.control.ledger.charged != 0 && disk.executions == 0 {
                producer.publish(1, changed_inputs.clone(), ElapsedTick(1000)).unwrap(); changed = true;
            }
            ElapsedTick(1000)
        }).unwrap();
        human.join().unwrap(); assert!(changed); assert_eq!(executed(&result), !change_opaque);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, u64::from(!change_opaque)); assert_eq!(disk.control.ledger.reserved, 0);
        assert_eq!(disk.control.ledger.charged, if change_opaque { 0 } else { 7 });
    }
}
