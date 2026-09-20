//! One managed source produces actual snapshots AND the notifications consumed
//! by the original file/two-key gate. Fixtures do not authenticate external truth.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod fixture;
use fixture::{Directory, Keys, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{FileCaptureError, PublicationInputFile};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::PublicationFeedFile;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::FilePublicationInputs;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::{
    FilePublicationProducer, PublicationProducerImage, PublicationProducerProfile, ProducerPublicationKind, MAX_PRODUCER_BYTES,
};
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::PublicationFreshnessPolicy;
use fa_reference::witness::refinement::index::routing::{RoutingBudget, WitnessChange};
use fa_reference::Error;

fn producer_profile() -> PublicationProducerProfile {
    PublicationProducerProfile { source: fixture::SOURCE, scope: profile().delivery.scope, feed: 41,
        clock_domain: profile().delivery.clock_domain, after: 0 }
}
fn empty() -> FilePublicationInputs { FilePublicationInputs::new(None, None) }
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }

struct Live {
    host: FileOversight,
    producer: FilePublicationProducer,
    keys: Keys,
    _reviewer: FileHumanReviewer,
}
impl Live {
    fn new(root: &Directory) -> Self {
        let (mut host, reviewer) = fixture::source_host(root);
        host.enable_publication_changes(host.revision(), PublicationChangePolicy { source: 41, after: 0,
            lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 } }).unwrap();
        host.enable_publication_change_freshness(host.revision(), PublicationFreshnessPolicy {
            clock_domain: profile().delivery.clock_domain, max_age_ticks: 3,
        }).unwrap();
        let (action, inputs) = fixture::reviewed(&mut host, 1, b"visible");
        let (producer, _) = FilePublicationProducer::create(root.0.join("producer"), producer_profile(),
            fixture::observations(&inputs, 1, &[0, 2, 4]), ElapsedTick(1)).unwrap();
        host.refresh_publication_feed(host.revision(), &producer.feed_reader().unwrap(), || ElapsedTick(1)).unwrap().unwrap();
        let original = producer.witness_reader(1, &action).unwrap().read_capture().unwrap();
        host.bind_publication_file_source(host.revision(), 1, original, fixture::requests()).unwrap();
        host.refresh_publication_from_file(host.revision(), 1, &producer.witness_reader(1, &action).unwrap()).unwrap().unwrap();
        let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
        let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
        let revision = host.revision();
        let human = reviewer.approve(&mut host, revision, &request).unwrap();
        Self { host, producer, keys: Keys { action, inputs, automatic, human, request }, _reviewer: reviewer }
    }
    fn refresh(&mut self, now: u64) {
        self.host.refresh_publication_feed(self.host.revision(), &self.producer.feed_reader().unwrap(),
            || ElapsedTick(now)).unwrap().unwrap();
        self.host.refresh_publication_from_file(self.host.revision(), 1,
            &self.producer.witness_reader(1, &self.keys.action).unwrap()).unwrap().unwrap();
    }
    fn dispatch(&mut self) -> Result<(), JournalError> {
        self.host.dispatch(self.host.revision(), &self.keys.automatic, &self.keys.human,
            &self.keys.action, &self.keys.inputs, snapshot())
    }
}

#[test]
fn derived_unrelated_notice_and_caught_up_image_publish_through_the_original_keys() {
    let root = Directory::new(); let mut live = Live::new(&root);
    let changed = fixture::observations(&live.keys.inputs, 2, &[0, 2, 4, 99]);
    let report = live.producer.publish(1, changed, ElapsedTick(2)).unwrap();
    assert_eq!(report.through, 1); assert_eq!(report.input_generation, 2);
    assert!(matches!(live.producer.image().batch().records()[0].change, WitnessChange::Key { key: 99, .. }));
    live.refresh(2); live.dispatch().unwrap(); live.refresh(2);
    let result = live.host.publish_checked(live.host.revision(), 1, Some(&live.keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    live.host.reconcile(live.host.revision(), 1).unwrap();
    assert_eq!(live.host.inspect().executions, 1);
    assert_eq!(live.host.inspect().control.ledger.charged, 16);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), live.host.inspect());
}

#[test]
fn every_generated_negative_change_after_dispatch_seals_without_rebasing_the_review() {
    for rows in [vec![2, 4], vec![0, 1, 2, 4], vec![0, 2, 3, 4], vec![0, 2, 4, 7]] {
        let root = Directory::new(); let mut live = Live::new(&root);
        live.refresh(1); live.dispatch().unwrap();
        live.producer.publish(1, fixture::observations(&live.keys.inputs, 2, &rows), ElapsedTick(2)).unwrap();
        live.refresh(2);
        let result = live.host.publish_checked(live.host.revision(), 1, Some(&live.keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
        assert_eq!(result.basis, PublicationBasis::Rejected(Error::Stale)); assert_eq!(result.outcome, sealed());
        assert_eq!(live.host.inspect().control.ledger.charged, 16);
        live.host.reconcile(live.host.revision(), 1).unwrap();
        assert_eq!(live.host.inspect().control.ledger.available, 100);
        assert_eq!(live.host.inspect().executions, 0);
    }
}

#[test]
fn rereading_a_stalled_producer_does_not_renew_its_heartbeat_but_new_observation_does() {
    let root = Directory::new(); let mut live = Live::new(&root);
    live.refresh(4);
    assert_eq!(live.dispatch(), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(live.host.inspect().control.ledger.reserved, 16);
    let inputs = live.producer.image().inputs().clone();
    let report = live.producer.publish(1, inputs, ElapsedTick(4)).unwrap();
    assert_eq!(report.input_generation, 1); assert_eq!(report.through, 0);
    live.refresh(4); live.dispatch().unwrap();
    assert_eq!(live.host.inspect().control.ledger.charged, 16);
}

#[test]
fn a_bundle_replacement_between_reads_exposes_a_future_cut_not_false_current_coverage() {
    let root = Directory::new(); let mut live = Live::new(&root);
    live.host.refresh_publication_feed(live.host.revision(), &live.producer.feed_reader().unwrap(),
        || ElapsedTick(1)).unwrap().unwrap();
    live.producer.publish(1, fixture::observations(&live.keys.inputs, 2, &[0, 2, 4, 99]), ElapsedTick(2)).unwrap();
    let read = live.host.refresh_publication_from_file(live.host.revision(), 1,
        &live.producer.witness_reader(1, &live.keys.action).unwrap());
    assert_eq!(read, Err(JournalError::Contract(Error::Incomplete)));
    assert!(live.host.storage_failure().is_some());
    assert_eq!(live.host.inspect().control.ledger.stages[&1], ActionState::Authorized);
    assert_eq!(live.host.inspect().executions, 0);
}

#[test]
fn exact_retry_and_pinned_reopen_preserve_generations_and_validate_before_cleanup() {
    let root = Directory::new(); let path = root.0.join("producer");
    let (mut producer, _) = FilePublicationProducer::create(&path, producer_profile(), empty(), ElapsedTick(1)).unwrap();
    assert!(matches!(FilePublicationProducer::open(&path, producer_profile(), 1), Err(JournalError::Busy)));
    producer.publish(1, empty(), ElapsedTick(2)).unwrap();
    let bytes = producer.image().to_bytes().unwrap();
    assert_eq!(producer.publish(1, empty(), ElapsedTick(2)).unwrap().kind, ProducerPublicationKind::AlreadyCurrent);
    assert_eq!(producer.image().to_bytes().unwrap(), bytes);
    assert_eq!(producer.publish(1, empty(), ElapsedTick(3)), Err(JournalError::Contract(Error::Binding)));
    assert!(producer.failure().is_none()); drop(producer);
    std::fs::write(path.join("delivery.pending"), b"inert").unwrap();
    let wrong = PublicationProducerProfile { feed: 42, ..producer_profile() };
    assert!(matches!(FilePublicationProducer::open(&path, wrong, 1), Err(JournalError::Contract(Error::Binding))));
    assert!(path.join("delivery.pending").exists());
    assert!(matches!(FilePublicationProducer::open(&path, producer_profile(), 3), Err(JournalError::Contract(Error::Stale))));
    assert!(path.join("delivery.pending").exists());
    let reopened = FilePublicationProducer::open(&path, producer_profile(), 2).unwrap();
    assert_eq!(reopened.image().to_bytes().unwrap(), bytes);
    assert!(!path.join("delivery.pending").exists());
}

#[test]
fn failed_replace_cannot_acknowledge_half_a_pair_or_reuse_a_poisoned_writer() {
    let root = Directory::new(); let path = root.0.join("producer");
    let (mut producer, _) = FilePublicationProducer::create(&path, producer_profile(), empty(), ElapsedTick(1)).unwrap();
    let bytes = producer.image().to_bytes().unwrap();
    std::fs::write(path.join("delivery.pending"), b"stage obstruction").unwrap();
    assert!(matches!(producer.publish(1, empty(), ElapsedTick(2)), Err(JournalError::Io(_))));
    assert!(producer.failure().is_some());
    assert_eq!(std::fs::read(path.join("delivery.bin")).unwrap(), bytes);
    assert_eq!(producer.publish(1, empty(), ElapsedTick(2)), Err(JournalError::Unavailable));
    assert!(matches!(producer.feed_reader(), Err(JournalError::Unavailable)));
    drop(producer);
    let mut reopened = FilePublicationProducer::open(&path, producer_profile(), 1).unwrap();
    assert_eq!(reopened.image().generation(), 1);
    assert_eq!(reopened.publish(1, empty(), ElapsedTick(2)).unwrap().generation, 2);
}

#[test]
fn out_of_band_replacement_is_detected_instead_of_overwritten_or_called_a_retry() {
    let root = Directory::new(); let path = root.0.join("producer");
    let (mut producer, _) = FilePublicationProducer::create(&path, producer_profile(), empty(), ElapsedTick(1)).unwrap();
    let foreign_bytes = producer.image().advance(1, empty(), ElapsedTick(3)).unwrap().to_bytes().unwrap();
    std::fs::write(path.join("delivery.bin"), &foreign_bytes).unwrap();
    assert_eq!(producer.publish(1, empty(), ElapsedTick(2)), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(std::fs::read(path.join("delivery.bin")).unwrap(), foreign_bytes);
    assert_eq!(producer.image().generation(), 1); assert!(producer.failure().is_some());
}

#[test]
fn explicit_reader_profiles_reject_foreign_bundles_legacy_downgrades_and_oversize_data() {
    let root = Directory::new(); let live = Live::new(&root);
    let path = root.0.join("producer/delivery.bin");
    let wrong = PublicationProducerProfile { after: 1, ..producer_profile() };
    let wrong_feed = PublicationFeedFile::from_producer(&path, wrong).unwrap();
    assert_eq!(wrong_feed.read_batch(), Err(FileCaptureError::Data(Error::Binding)));
    let wrong_input = PublicationInputFile::from_producer(&path, wrong, 1, &live.keys.action).unwrap();
    assert_eq!(wrong_input.read_capture(), Err(FileCaptureError::Data(Error::Binding)));
    assert!(PublicationFeedFile::new(&path, 41).unwrap().read_batch().is_err());
    assert!(PublicationInputFile::new(&path, fixture::SOURCE).unwrap().read_capture().is_err());
    let bytes = live.producer.image().to_bytes().unwrap();
    let copy = root.0.join("copy.bin"); std::fs::write(&copy, &bytes).unwrap();
    let input = PublicationInputFile::from_producer(&copy, producer_profile(), 1, &live.keys.action).unwrap();
    let feed = PublicationFeedFile::from_producer(&copy, producer_profile()).unwrap();
    assert_eq!(input.read_capture().unwrap().input_cut().unwrap().through, feed.read_batch().unwrap().heartbeat().through);
    std::fs::write(&copy, vec![0; MAX_PRODUCER_BYTES + 1]).unwrap();
    assert_eq!(input.read_capture(), Err(FileCaptureError::Data(Error::Limit)));
    assert_eq!(feed.read_batch(), Err(FileCaptureError::Data(Error::Limit)));
    assert_eq!(PublicationProducerImage::from_bytes(&bytes).unwrap(), *live.producer.image());
}
