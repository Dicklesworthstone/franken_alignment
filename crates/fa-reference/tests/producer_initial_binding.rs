//! Initial requirements use one actual producer image, not a genesis-only cut.
//! Real file/native owner tests; neither source truth nor hardware crashes proved.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod fixture;
use fixture::{Directory, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick, FrozenAction};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::PublicationInputFile;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::PublicationFeedFile;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::{FilePublicationProducer, PublicationProducerProfile};
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::PublicationFreshnessPolicy;
use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::witness::refinement::index::routing::RoutingBudget;
use fa_reference::Error;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn identity() -> PublicationProducerProfile {
    PublicationProducerProfile { source: fixture::SOURCE, scope: profile().delivery.scope,
        feed: 41, clock_domain: profile().delivery.clock_domain, after: 0 }
}
fn owner(root: &Directory) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_with_publication_change_freshness(root.store(),
        profile(), fixture::limits(), PublicationChangePolicy { source: 41, after: 0,
            lookup: RoutingBudget { steps: 10000, bytes: 1000000 } },
        PublicationFreshnessPolicy { clock_domain: profile().delivery.clock_domain, max_age_ticks: 10 }).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}
fn propose(host: &mut FileOversight, id: u64) -> (FrozenAction, CommitteeInput) {
    let action = host.propose(host.revision(), id, fixture::spec(host, b"visible"), snapshot()).unwrap();
    let inputs = fixture::inputs(&action, b"complete source view");
    (action, inputs)
}
fn producer(root: &Directory, inputs: &CommitteeInput) -> FilePublicationProducer {
    FilePublicationProducer::create(root.0.join("producer"), identity(),
        fixture::observations(inputs, 1, &[0, 2, 4]), ElapsedTick(1)).unwrap().0
}
fn refresh(host: &mut FileOversight, witness: &PublicationInputFile, feed: &PublicationFeedFile) {
    host.refresh_publication_from_producer(host.revision(), 1, witness, feed, || ElapsedTick(2)).unwrap().unwrap();
}

#[test]
fn an_already_advanced_producer_can_bind_then_complete_with_fresh_original_keys() {
    let root = Directory::new(); let (mut host, reviewer) = owner(&root);
    let (action, inputs) = propose(&mut host, 1); let mut producer = producer(&root, &inputs);
    producer.publish(1, fixture::observations(&inputs, 2, &[0, 2, 4, 99]), ElapsedTick(2)).unwrap();
    producer.publish(2, fixture::observations(&inputs, 3, &[0, 2, 4, 99, 100]), ElapsedTick(2)).unwrap();
    let witness = producer.witness_reader(1, &action).unwrap(); let feed = producer.feed_reader().unwrap();
    let before = host.revision();
    // This is the real former startup blocker: the snapshot is ahead of the
    // untouched bootstrap feed. Do not weaken that strict native admission law.
    assert_eq!(host.bind_publication_file_source(before, 1, witness.read_capture().unwrap(), fixture::requests()),
        Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.revision(), before);
    let (observed, report) = host.bind_publication_from_producer(before, 1, &witness, &feed,
        fixture::requests(), || ElapsedTick(2)).unwrap().unwrap();
    assert_eq!(observed.generation, 3); assert_eq!(report.before, 0);
    assert_eq!(report.status.through, 2); assert_eq!(report.changes.len(), 2);
    assert!(report.status.complete()); assert_eq!(host.revision(), before + 5);
    assert_eq!(host.retained_publication_evidence(1).unwrap().original(), producer.image().inputs());
    let source = host.publication_source(1).unwrap().unwrap();
    assert!(!source.fresh); assert!(!source.capture_pending);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    fixture::review_existing(&mut host, 1, 101, &inputs);
    assert!(matches!(host.authorize(host.revision(), 1, &inputs, snapshot()),
        Err(JournalError::Contract(Error::Incomplete))));
    refresh(&mut host, &witness, &feed);
    let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    refresh(&mut host, &witness, &feed);
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    refresh(&mut host, &witness, &feed);
    assert_eq!(host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
}

#[test]
fn replacement_after_the_single_read_cannot_change_the_original_requirements() {
    for added in [99, 1] {
        let root = Directory::new(); let (mut host, _) = owner(&root);
        let (action, inputs) = propose(&mut host, 1); let mut producer = producer(&root, &inputs);
        let original = producer.image().inputs().clone();
        let witness = producer.witness_reader(1, &action).unwrap(); let feed = producer.feed_reader().unwrap();
        let mut calls = 0;
        let (_, report) = host.bind_publication_from_producer(host.revision(), 1, &witness, &feed,
            fixture::requests(), || {
                calls += 1;
                producer.publish(1, fixture::observations(&inputs, 2, &[0, 2, 4, added]), ElapsedTick(2)).unwrap();
                ElapsedTick(2)
            }).unwrap().unwrap();
        assert_eq!(calls, 1); assert_eq!(report.status.through, 0);
        assert_eq!(host.retained_publication_evidence(1).unwrap().original(), &original);
        fixture::review_existing(&mut host, 1, 101, &inputs);
        refresh(&mut host, &witness, &feed);
        let result = host.authorize(host.revision(), 1, &inputs, snapshot());
        if added == 99 { assert!(result.is_ok()); }
        else { assert!(matches!(result, Err(JournalError::Contract(Error::Stale)))); }
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.reserved, if added == 99 { 16 } else { 0 });
        assert_eq!(host.retained_publication_evidence(1).unwrap().original(), &original);
    }
}

#[test]
fn lost_or_malformed_read_withdraws_the_feed_and_can_retry_without_binding_any_recipe() {
    for malformed in [false, true] {
        let root = Directory::new(); let (mut host, _) = owner(&root);
        let (action, inputs) = propose(&mut host, 1); let producer = producer(&root, &inputs);
        let witness = producer.witness_reader(1, &action).unwrap(); let feed = producer.feed_reader().unwrap();
        let path = root.0.join("producer/delivery.bin"); let bytes = std::fs::read(&path).unwrap();
        if malformed { std::fs::write(&path, b"not a producer").unwrap(); }
        else { std::fs::remove_file(&path).unwrap(); }
        let before = host.revision(); let clock = Cell::new(0);
        let result = host.bind_publication_from_producer(before, 1, &witness, &feed, fixture::requests(), || {
            clock.set(clock.get() + 1); ElapsedTick(2)
        });
        assert!(matches!(result, Ok(Err(_)))); assert_eq!(clock.get(), 0);
        assert_eq!(host.revision(), before + 1); assert!(host.storage_failure().is_none());
        assert_eq!(host.publication_source(1).unwrap(), None);
        assert!(matches!(host.retained_publication_evidence(1), Err(JournalError::Contract(Error::Missing))));
        std::fs::write(&path, bytes).unwrap();
        assert!(host.bind_publication_from_producer(host.revision(), 1, &witness, &feed,
            fixture::requests(), || ElapsedTick(2)).unwrap().is_ok());
    }
}

#[test]
fn duplicate_foreign_and_active_review_preflights_do_not_read_or_advance_history() {
    let root = Directory::new(); let (mut host, _) = owner(&root);
    let (action, inputs) = propose(&mut host, 1); let producer = producer(&root, &inputs);
    let witness = producer.witness_reader(1, &action).unwrap(); let feed = producer.feed_reader().unwrap();
    let foreign = producer.witness_reader(2, &action).unwrap();
    let revision = host.revision();
    assert_eq!(host.bind_publication_from_producer(revision, 1, &foreign, &feed,
        fixture::requests(), || panic!("foreign preflight must not sample time")), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.revision(), revision);
    let raw = PublicationInputFile::new(root.0.join("missing"), fixture::SOURCE).unwrap();
    assert!(host.bind_publication_from_producer(revision, 1, &raw, &feed,
        fixture::requests(), || panic!("raw reader preflight")).is_err());
    assert_eq!(host.revision(), revision);
    host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, fixture::ROOT, fixture::window(&host), snapshot()).unwrap();
    let revision = host.revision();
    assert_eq!(host.bind_publication_from_producer(revision, 1, &witness, &feed,
        fixture::requests(), || panic!("active review preflight")), Err(JournalError::Contract(Error::WrongState)));
    assert_eq!(host.revision(), revision);
    // Finish using the original helper protocol, then bind once. The operation
    // never overwrites requirements even when the original file later vanishes.
    fixture::votes(&mut host, 101, fa_reference::round::Verdict::Allow);
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    host.bind_publication_from_producer(host.revision(), 1, &witness, &feed,
        fixture::requests(), || ElapsedTick(2)).unwrap().unwrap();
    let revision = host.revision(); std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    assert_eq!(host.bind_publication_from_producer(revision, 1, &witness, &feed,
        fixture::requests(), || panic!("duplicate preflight")), Err(JournalError::Contract(Error::Duplicate)));
    assert_eq!(host.revision(), revision); assert!(host.storage_failure().is_none());
}

#[test]
fn invalid_original_witnesses_do_not_commit_only_the_catch_up_half() {
    let root = Directory::new(); let (mut host, _) = owner(&root);
    let (action, inputs) = propose(&mut host, 1); let mut producer = producer(&root, &inputs);
    producer.publish(1, fixture::observations(&inputs, 2, &[0, 1, 2, 4]), ElapsedTick(2)).unwrap();
    let witness = producer.witness_reader(1, &action).unwrap(); let feed = producer.feed_reader().unwrap();
    let mut before = host.inspect(); before.revision += 1;
    assert_eq!(host.bind_publication_from_producer(host.revision(), 1, &witness, &feed,
        fixture::requests(), || ElapsedTick(2)), Err(JournalError::Contract(Error::Binding)));
    assert!(host.storage_failure().is_some());
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), before);
    drop(host);
    let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.publication_change_status().unwrap().through, 0);
    assert_eq!(host.publication_source(1).unwrap(), None);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
}

#[test]
fn a_missing_retained_prefix_is_not_filled_from_the_new_original_snapshot() {
    for changes in [256, 257] {
        let root = Directory::new(); let (mut host, _) = owner(&root);
        let (action, inputs) = propose(&mut host, 1); let producer = producer(&root, &inputs);
        let mut image = producer.image().clone();
        for i in 1..=changes {
            let keys: &[u64] = if i % 2 == 0 { &[0, 2, 4] } else { &[0, 2, 4, 99] };
            image = image.advance(image.generation(), fixture::observations(&inputs, i + 1, keys), ElapsedTick(2)).unwrap();
        }
        let path = root.0.join("retained.bin"); std::fs::write(&path, image.to_bytes().unwrap()).unwrap();
        let witness = PublicationInputFile::from_producer(&path, identity(), 1, &action).unwrap();
        let feed = PublicationFeedFile::from_producer(&path, identity()).unwrap();
        let result = host.bind_publication_from_producer(host.revision(), 1, &witness, &feed,
            fixture::requests(), || ElapsedTick(2));
        if changes == 256 {
            let (_, report) = result.unwrap().unwrap();
            assert_eq!(report.status.through, 256); assert_eq!(report.changes.len(), 256);
        } else {
            assert_eq!(result, Err(JournalError::Contract(Error::Incomplete)));
            assert!(host.storage_failure().is_some());
        }
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.reserved, 0);
    }
}

#[test]
fn clock_unwind_and_failed_replacement_acknowledge_neither_binding_nor_feed() {
    for panic_clock in [false, true] {
        let root = Directory::new(); let (mut host, _) = owner(&root);
        let (action, inputs) = propose(&mut host, 1); let mut producer = producer(&root, &inputs);
        producer.publish(1, fixture::observations(&inputs, 2, &[0, 2, 4, 99]), ElapsedTick(2)).unwrap();
        let witness = producer.witness_reader(1, &action).unwrap(); let feed = producer.feed_reader().unwrap();
        let mut withdrawn = host.inspect(); withdrawn.revision += 1;
        let result = catch_unwind(AssertUnwindSafe(|| host.bind_publication_from_producer(host.revision(),
            1, &witness, &feed, fixture::requests(), || {
                assert!(!panic_clock, "injected post-read clock unwind");
                std::fs::write(root.store().join("delivery.pending"), b"obstruct final stage").unwrap();
                ElapsedTick(2)
            })));
        if panic_clock { assert!(result.is_err()); }
        else { assert!(matches!(result.unwrap(), Err(JournalError::Io(_)))); }
        assert!(host.storage_failure().is_some());
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), withdrawn);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(host.bind_publication_from_producer(host.revision(), 1, &witness, &feed,
            fixture::requests(), || ElapsedTick(2)), Err(JournalError::Unavailable));
        drop(host);
        let (reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
        assert_eq!(reopened.publication_source(1).unwrap(), None);
        assert_eq!(reopened.publication_change_status().unwrap().through, 0);
    }
}

#[test]
fn empty_recipes_require_and_retain_the_whole_opaque_input() {
    use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::FilePublicationInputs;
    for with_opaque in [false, true] {
        let root = Directory::new(); let (mut host, _) = owner(&root);
        let (action, inputs) = propose(&mut host, 1);
        let base = fixture::observations(&inputs, 1, &[0, 2, 4]);
        let opaque = with_opaque.then(|| inputs.views()["alpha"].actual_input().clone());
        let original = FilePublicationInputs::new(base.structured().cloned(), opaque);
        let (producer, _) = FilePublicationProducer::create(root.0.join("producer"), identity(),
            original.clone(), ElapsedTick(1)).unwrap();
        let witness = producer.witness_reader(1, &action).unwrap(); let feed = producer.feed_reader().unwrap();
        let result = host.bind_publication_from_producer(host.revision(), 1, &witness, &feed,
            vec![], || ElapsedTick(2));
        if with_opaque {
            assert!(result.unwrap().is_ok());
            assert_eq!(host.retained_publication_evidence(1).unwrap().original(), &original);
            assert!(!host.publication_source(1).unwrap().unwrap().fresh);
            assert!(host.storage_failure().is_none());
        } else {
            assert_eq!(result, Err(JournalError::Contract(Error::Incomplete)));
            assert!(host.storage_failure().is_some());
        }
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.reserved, 0);
    }
}
