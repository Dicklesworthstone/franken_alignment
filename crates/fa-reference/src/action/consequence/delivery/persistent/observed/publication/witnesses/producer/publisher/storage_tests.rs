use super::*;
use crate::action::{Purpose, Scope};
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn every_replace_barrier_recovers_one_whole_source_pair_and_an_exact_retry() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-producer-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let profile = PublicationProducerProfile { source: 91, feed: 41, clock_domain: 1, after: 0,
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect } };
        let next = FilePublicationInputs::new(None, Some(ActualHelperInput::new(vec![1], InputProfileBinding {
            profile_id: 1, profile_bytes: vec![1], tokenizer_epoch: 1, model_epoch: 1, policy_epoch: 1,
        }, vec![SubmittedPart { span: ByteSpan { start: 0, end: 1 }, kind: PartKind::Question }], vec![]).unwrap()));
        let (mut producer, _) = FilePublicationProducer::create(&path, profile, FilePublicationInputs::new(None, None), ElapsedTick(1)).unwrap();
        producer.store.fail_once(barrier);
        assert!(matches!(producer.publish(1, next.clone(), ElapsedTick(2)), Err(JournalError::Io(_))));
        assert!(producer.failure().is_some());
        assert_eq!(producer.image().generation(), 1);
        drop(producer);
        let mut recovered = FilePublicationProducer::open(&path, profile, 1).unwrap();
        let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(recovered.image().generation(), if visible { 2 } else { 1 });
        assert_eq!(recovered.image().batch().heartbeat().through, u64::from(visible));
        assert_eq!(recovered.image().inputs().opaque().is_some(), visible);
        let retry = recovered.publish(1, next, ElapsedTick(2)).unwrap();
        assert_eq!(retry.kind, if visible { ProducerPublicationKind::AlreadyCurrent } else { ProducerPublicationKind::Replaced });
        assert_eq!(retry.generation, 2); assert_eq!(retry.input_generation, 2); assert_eq!(retry.through, 1);
        assert_eq!(recovered.image().batch().records().len(), 1);
        drop(recovered);
        std::fs::remove_dir_all(path).unwrap();
    }
}
