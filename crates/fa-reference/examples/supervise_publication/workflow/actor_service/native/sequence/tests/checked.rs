//! The resident handoff keeps the original fresh producer-witness gate.
use super::*;
use crate::config::CLOCK_DOMAIN;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationInputs, FileWitnessInput,
    producer::{FilePublicationProducer, PublicationProducerProfile},
};
use fa_reference::product_frontier::{
    FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};

pub(super) fn witness_inputs(reader: &Reader, revision: u64, keys: &[u64]) -> FilePublicationInputs {
    let key = ProjectionKey {
        source: 40,
        branch: reader.profile.delivery.scope.branch,
        projection: 7,
        source_epoch: 1,
    };
    let close = TrustedClosingMarker {
        key,
        final_sequence: 1,
        marker_generation: 1,
    };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers
        .accept(key, FrontierStage::Authenticated, 1)
        .unwrap();
    frontiers.record_close(close).unwrap();
    FilePublicationInputs::new(
        Some(
            FileWitnessInput::new(
                revision,
                revision,
                20,
                AdapterDomainInput::new(
                    DomainProjection::new(40, 1, key),
                    DomainClosure::Closed(close),
                ),
                keys.iter()
                    .map(|&key| SnapshotEntry::new(key, 1, b"original".to_vec()).unwrap())
                    .collect(),
                &frontiers,
            )
            .unwrap(),
        ),
        None,
    )
}

pub(super) fn publication(root: &Root, reader: &Reader) -> (PublicationProfile, FilePublicationProducer) {
    selected_publication(root, reader, false, false, ElapsedTick(1000), 5000)
}

pub(super) fn joint_publication(
    root: &Root,
    reader: &Reader,
) -> (PublicationProfile, FilePublicationProducer) {
    selected_publication(root, reader, true, false, ElapsedTick(1000), 5000)
}

/// Return the profile backed by the producer's original durable image.
pub(super) fn evaluated_publication(root: &Root, reader: &Reader, fallback: bool) -> PublicationProfile {
    selected_publication(root, reader, false, fallback, crate::workflow::clock(), 120_000).0
}

fn selected_publication(
    root: &Root,
    reader: &Reader,
    joint: bool,
    fallback: bool,
    now: ElapsedTick,
    max_age: u64,
) -> (PublicationProfile, FilePublicationProducer) {
    let scope = reader.profile.delivery.scope;
    let (producer, _) = FilePublicationProducer::create(
        root.0.join("producer"),
        PublicationProducerProfile {
            source: 91,
            scope,
            feed: 41,
            clock_domain: CLOCK_DOMAIN,
            after: 0,
        },
        witness_inputs(reader, 1, &[0]),
        now,
    )
    .unwrap();
    let document = witness_document(&root.0.join("producer/delivery.bin"), reader, fallback, max_age);
    let document = if joint {
        format!(
            r#"{{"schema":"fa.supervised-joint-publication/1","joint":{{"id":9,"generation":1,"minimum_safe_roots":2,"minimum_violation_roots":2,"maximum_escape_ppm":0,"maximum_false_stop_ppm":0,"max_cases":8,"max_member_outcomes":32}},"publication":{document}}}"#,
        )
    } else {
        document
    };
    (
        PublicationProfile::decode(document.as_bytes()).unwrap(),
        producer,
    )
}

pub(super) fn witness_document(
    path: &std::path::Path,
    reader: &Reader,
    fallback: bool,
    max_age: u64,
) -> String {
    let scope = reader.profile.delivery.scope;
    let (schema, history) = if fallback {
        ("fa.supervised-witnesses/5", r#","history":"exact_current_snapshot""#)
    } else {
        ("fa.supervised-witnesses/3", "")
    };
    format!(
        r#"{{"schema":"{schema}","source":91{history},"producer":{{"path":"{}","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":{max_age},"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":0,"role":"subject"}},{{"kind":"absent_key","key":1}}]}}"#,
        path.display(),
        scope.tenant,
        scope.principal,
        scope.run,
        scope.branch,
        scope.authority,
    )
}

#[test]
fn resident_checked_publication_rechecks_each_frame_and_preserves_the_confirmed_prefix() {
    for phantom in [false, true] {
        let root = Root::new();
        let (config, entries, reviewer) = selected(&root, true);
        let reader = Reader::new(&config, &entries[0]);
        let (publication, mut producer) = publication(&root, &reader);
        let mut output = Output::new(&entries);
        let profiles = output.profiles.clone();
        let mut humans = Humans::new(&reviewer, &reader, [Some(ReviewDecision::Approve); 3]);
        let mut first_charge = None;
        let mut changed = false;
        let result = run(
            config,
            entries,
            &reviewer,
            Some(&publication),
            || {
                humans.poll();
                if !changed
                    && let Ok(image) =
                        FileOversight::read_publication(&reader.store, &reader.profile)
                    && image.executions == 1
                {
                    let prior = *first_charge.get_or_insert(image.control.ledger.charged);
                    if image.control.ledger.charged > prior {
                        // Both cases reach the second ORIGINAL Dispatch after its
                        // real helper and human approval. Only the changed key
                        // differs: disjoint 99 preserves the witness; key 1 breaks
                        // the reviewed absence and must hold this frame.
                        let keys: &[u64] = if phantom { &[0, 1] } else { &[0, 99] };
                        producer
                            .publish(1, witness_inputs(&reader, 2, keys), ElapsedTick(1001))
                            .unwrap();
                        changed = true;
                    }
                }
                ElapsedTick(if changed { 1001 } else { 1000 })
            },
            &mut output,
        );
        assert!(
            changed,
            "the second real Dispatch must precede the producer mutation"
        );
        assert_eq!(output.documents.len(), if phantom { 2 } else { 3 });
        assert_eq!(humans.seen[2], !phantom);
        let packets = humans.join();
        let replies = output.join();
        assert!(executed(&replies[0]));
        assert_eq!(executed(&replies[1]), !phantom, "{result:?}");
        if phantom {
            assert!(result.is_err());
            let stream = reader.stream();
            assert_eq!(stream.publication.executions, 1);
            assert_eq!(stream.publication.payload, b"A");
            assert_eq!(stream.publication.control.ledger.epoch, 0);
            assert_eq!(stream.publication.control.ledger.reserved, 0);
            assert_eq!(
                stream.publication.control.ledger.charged,
                packets[0].as_ref().unwrap().action().spec().units
            );
            assert_eq!(stream.confirmed, stream.published);
            assert!(!stream.confirmed.finished());
            assert_eq!(reader.progress(8).numerical.numerical.position, 8);
            reader.no_generation(9);
        } else {
            assert!(result.is_ok(), "{result:?}");
            complete(&reader, &replies, &packets);
        }
        closed(&profiles, &reviewer);
    }
}
