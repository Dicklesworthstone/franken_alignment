//! Concrete native policy leases inside the original two-key publication cut.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod fixture;
use fixture::{Directory, Keys, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer, FileOversightProfile};
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::completion::{CapturedCompletionKeys, files::FilesCompletionReport};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::{PublicationFeedBatch, PublicationFeedFile};
use fa_reference::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationChangePolicy};
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::{PublicationFreshnessPolicy, PublicationHeartbeat};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceError, EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::witness::refinement::index::routing::{RoutingBudget, WitnessChange};
use fa_reference::Error;
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

const POLICY: u64 = 51;
const FEED: u64 = 41;
fn document(generation: u64, change: bool) -> EvidenceSnapshot {
    let mut state = snapshot();
    if change { state.values.insert(7, b"changed".to_vec()); }
    EvidenceSnapshot::new(EvidenceIdentity { source: POLICY, generation, scope: profile().delivery.scope },
        state, BTreeMap::from([("alpha".to_owned(), b"alpha evidence".to_vec()),
            ("beta".to_owned(), b"beta evidence".to_vec())])).unwrap()
}
fn write(path: &Path, bytes: &[u8]) {
    let pending = path.with_extension("next");
    std::fs::write(&pending, bytes).unwrap(); std::fs::rename(pending, path).unwrap();
}
fn feed(root: &Directory, generation: u64, through: u64, produced: u64) {
    let batch = PublicationFeedBatch::new(PublicationHeartbeat { source: FEED,
        clock_domain: profile().delivery.clock_domain, generation, through, produced_at: ElapsedTick(produced) },
        0, (1..=through).map(|sequence| PublicationChange { source: FEED, sequence, change: WitnessChange::All }).collect()).unwrap();
    write(&root.0.join("feed.bin"), &batch.to_bytes().unwrap());
}
fn keys(keys: &Keys) -> CapturedCompletionKeys<'_> {
    CapturedCompletionKeys { automatic: &keys.automatic, human: &keys.human, credential: None }
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
struct Ready {
    host: FileOversight,
    reviewer: FileHumanReviewer,
    source: FileEvidenceSource,
    keys: Keys,
    root: Directory,
}
impl Ready {
    fn new(with_feed: bool) -> Self { Self::configured(profile(), true, with_feed) }
    fn configured(p: FileOversightProfile, native: bool, with_feed: bool) -> Self {
        let root = Directory::new();
        let (mut host, reviewer) = FileOversight::create_with_publication_validation(root.store(), p, fixture::limits()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        if with_feed {
            host.enable_publication_changes(host.revision(), PublicationChangePolicy {
                source: FEED, after: 0, lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 },
            }).unwrap();
            host.enable_publication_change_freshness(host.revision(), PublicationFreshnessPolicy {
                clock_domain: profile().delivery.clock_domain, max_age_ticks: 20,
            }).unwrap();
            feed(&root, 1, 0, 1);
            let reader = PublicationFeedFile::new(root.0.join("feed.bin"), FEED).unwrap();
            host.refresh_publication_feed(host.revision(), &reader, || ElapsedTick(1)).unwrap().unwrap();
        }
        let path = root.0.join("policy.json"); write(&path, &document(1, false).encode());
        let mut source = FileEvidenceSource::new(&path, POLICY, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
        if native {
            host.enable_file_source(host.revision(), FileSourcePolicy {
                source: StateSource { source: POLICY, scope: profile().delivery.scope, generation: 1 },
                limits: StateLimits::default(), freshness: StateFreshness::new(3).unwrap(),
            }).unwrap();
            host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
        }
        let action = host.propose(host.revision(), 1, fixture::spec(&host, b"visible"), snapshot()).unwrap();
        let inputs = document(1, false).inputs_for(&action, &profile().committee).unwrap();
        fixture::review_existing(&mut host, 1, 101, &inputs);
        let original = fixture::packet(1, &action, &inputs, 1, &[0, 2, 4]);
        fixture::replace_source(&root, &original);
        host.bind_publication_file_source(host.revision(), 1, original, fixture::requests()).unwrap();
        fixture::refresh(&mut host, &root, 1);
        let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
        let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
        let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
        Self { host, reviewer, source, keys: Keys { action, inputs, automatic, human, request }, root }
    }
    fn complete(&mut self, now: u64) -> FilesCompletionReport {
        self.host.complete_publication_from_files(self.host.revision(), keys(&self.keys),
            &mut self.source, &fixture::source(&self.root), None, || ElapsedTick(now))
    }
}

#[test]
fn concrete_rereads_refresh_expired_native_leases_while_the_callback_path_does_not() {
    let mut callback = Ready::new(false);
    let inputs = callback.keys.inputs.clone();
    let refused = callback.host.complete_publication_from_source(callback.host.revision(), keys(&callback.keys),
        &fixture::source(&callback.root), || ElapsedTick(5), |_, _| Ok(DriverEvidence {
            inputs: Some(inputs.clone()), snapshot: snapshot(),
        }));
    assert_eq!(refused.result, Err(JournalError::Contract(Error::Stale)));
    assert_eq!(callback.host.inspect().executions, 0);
    let mut r = Ready::new(false);
    let before = r.host.revision(); let retained = r.host.file_source_status().unwrap().capture.retained_events;
    let report = r.complete(5);
    assert_eq!(report.observations, vec![Ok(document(1, false).identity()); 2]);
    assert_eq!(report.committed_source_updates, vec![Ok(document(1, false).identity()); 2]);
    let publication = report.completion.completion.result.unwrap();
    assert_eq!(publication.basis, PublicationBasis::Revalidated);
    assert_eq!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(r.host.revision(), before + 10);
    assert_eq!(r.host.file_source_status().unwrap().capture.retained_events, retained + 2);
    assert!(!r.host.file_source_status().unwrap().interrupted);
    assert_eq!(r.host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(r.host.inspect().control.ledger.charged, 16);
    assert_eq!(FileOversight::read_publication(r.root.store(), &profile()).unwrap(), r.host.inspect());
}

#[test]
fn source_generation_or_policy_change_after_dispatch_cannot_rebase_existing_review() {
    for changed_value in [false, true] {
        let mut r = Ready::new(false); let root = &r.root; let mut clocks = 0;
        let report = r.host.complete_publication_from_files(r.host.revision(), keys(&r.keys), &mut r.source,
            &fixture::source(root), None, || {
                clocks += 1;
                if clocks == 3 { write(&root.0.join("policy.json"), &document(2, changed_value).encode()); }
                // Even after the candidate dispatch the canonical ledger is still
                // undispatched. Source updates cannot leak out on their own.
                let visible = FileOversight::read_publication(root.store(), &profile()).unwrap();
                assert_eq!(visible.executions, 0); assert_eq!(visible.control.ledger.reserved, 16);
                ElapsedTick(2)
            });
        assert_eq!(clocks, 4);
        assert_eq!(report.observations, vec![Ok(document(1, false).identity()), Ok(document(2, changed_value).identity())]);
        assert_eq!(report.committed_source_updates.len(), 2);
        let outcome = report.completion.completion.result.unwrap();
        assert!(matches!(outcome.basis, PublicationBasis::Rejected(_)));
        assert_eq!(outcome.outcome, sealed());
        assert_eq!(r.host.file_source_status().unwrap().producer, Some(document(2, changed_value).identity()));
        assert_eq!(r.host.inspect().control.ledger.available, 100);
        assert_eq!(r.host.inspect().executions, 0);
    }
}

#[test]
fn lost_second_policy_read_withdraws_native_source_and_settles_nonexecution() {
    let mut r = Ready::new(false); let root = &r.root; let mut clocks = 0;
    let report = r.host.complete_publication_from_files(r.host.revision(), keys(&r.keys), &mut r.source,
        &fixture::source(root), None, || {
            clocks += 1; if clocks == 3 { std::fs::remove_file(root.0.join("policy.json")).unwrap(); }
            ElapsedTick(2)
        });
    assert_eq!(report.observations[1], Err(EvidenceError::Io(std::io::ErrorKind::NotFound)));
    assert_eq!(report.committed_source_updates, vec![Ok(document(1, false).identity()), Err(Error::Incomplete)]);
    assert_eq!(report.completion.completion.evidence_failure, Some(Error::Incomplete));
    assert_eq!(report.completion.completion.result.unwrap().outcome, sealed());
    assert_eq!(r.host.inspect().control.ledger.stages[&1], ActionState::ConfirmedNotExecuted);
    assert_eq!(r.host.inspect().control.ledger.available, 100);
    assert_eq!(r.host.reconcile(r.host.revision(), 1), Ok(Reconciliation::Resolved(sealed())));
}

#[test]
fn first_policy_loss_is_not_acknowledged_as_a_source_update_or_healthy_old_basis() {
    let mut r = Ready::new(false); let before = r.host.inspect();
    std::fs::remove_file(r.root.0.join("policy.json")).unwrap();
    let report = r.complete(2);
    assert_eq!(report.completion.completion.result, Err(JournalError::Contract(Error::Incomplete)));
    assert!(report.committed_source_updates.is_empty()); assert!(report.completion.committed.is_empty());
    assert!(r.host.storage_failure().is_some()); assert!(r.host.file_source_status().unwrap().interrupted);
    assert_eq!(r.host.inspect().control.ledger.reserved, 16);
    assert_eq!(r.host.revision(), before.revision + 1);
    drop(r.host); drop(r.reviewer);
    let (recovered, _) = FileOversight::open_with_publication_validation(r.root.store(), profile(), fixture::limits()).unwrap();
    assert_eq!(recovered.inspect().executions, 0);
    assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Cancelled);
}

#[test]
fn native_lease_starts_before_capture_and_both_effect_checks_enforce_its_exact_expiry() {
    for boundary in [2, 4] {
        for final_tick in [3, 4] {
            let mut r = Ready::new(false); let mut clocks = 0;
            let report = r.host.complete_publication_from_files(r.host.revision(), keys(&r.keys), &mut r.source,
                &fixture::source(&r.root), None, || {
                    clocks += 1; ElapsedTick(if clocks >= boundary { final_tick } else { 1 })
                });
            if final_tick == 3 {
                assert_eq!(report.completion.completion.result.unwrap().outcome, EndpointOutcome::Executed { resulting_version: 2 });
            } else if boundary == 2 {
                assert_eq!(report.completion.completion.result, Err(JournalError::Contract(Error::Stale)));
                assert!(report.committed_source_updates.is_empty()); assert!(r.host.storage_failure().is_some());
            } else {
                assert_eq!(report.completion.completion.result.unwrap().outcome, sealed());
                assert_eq!(r.host.inspect().control.ledger.available, 100);
            }
            assert_eq!(r.host.inspect().executions, u64::from(final_tick == 3));
        }
    }
}

#[test]
fn feed_catchup_and_native_policy_refresh_share_the_original_settled_cut() {
    let mut r = Ready::new(true); feed(&r.root, 2, 1, 5);
    let before = r.host.revision();
    let feed_file = PublicationFeedFile::new(r.root.0.join("feed.bin"), FEED).unwrap();
    let report = r.host.complete_publication_from_files(before, keys(&r.keys), &mut r.source,
        &fixture::source(&r.root), Some(&feed_file), || ElapsedTick(5));
    assert_eq!(report.completion.reads.len(), 2); assert_eq!(report.observations.len(), 2);
    assert_eq!(report.completion.committed[0].changes.len(), 1);
    assert!(report.completion.committed[1].changes.is_empty());
    assert_eq!(report.completion.completion.result.unwrap().outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(r.host.revision(), before + 15);
    assert_eq!(r.host.publication_change_status().unwrap().through, 1);
    assert_eq!(FileOversight::read_publication(r.root.store(), &profile()).unwrap(), r.host.inspect());
}

#[test]
fn failed_final_write_exposes_no_speculative_source_refresh_or_receipt() {
    let mut r = Ready::new(false); let before = r.host.inspect();
    let retained = r.host.file_source_status().unwrap().capture.retained_events;
    let root = &r.root; let mut clocks = 0;
    let report = r.host.complete_publication_from_files(before.revision, keys(&r.keys), &mut r.source,
        &fixture::source(root), None, || {
            clocks += 1;
            if clocks == 4 { std::fs::write(root.store().join("delivery.pending"), b"stage obstruction").unwrap(); }
            ElapsedTick(5)
        });
    assert!(matches!(report.completion.completion.result, Err(JournalError::Io(_))));
    assert_eq!(report.observations.len(), 2); assert!(report.committed_source_updates.is_empty());
    assert_eq!(r.host.file_source_status().unwrap().capture.retained_events, retained);
    assert!(r.host.file_source_status().unwrap().interrupted); assert!(r.host.storage_failure().is_some());
    assert_eq!(FileOversight::read_publication(r.root.store(), &profile()).unwrap(), r.host.inspect());
    assert_eq!(r.host.inspect().executions, 0); assert_eq!(r.host.inspect().control.ledger.reserved, 16);
}

#[test]
fn clock_unwind_before_either_policy_read_cannot_reopen_original_admission() {
    for panic_at in [1, 3] {
        let mut r = Ready::new(false); let before = r.host.revision(); let mut calls = 0;
        let reads = r.source.status().read_attempts;
        assert!(catch_unwind(AssertUnwindSafe(|| {
            r.host.complete_publication_from_files(before, keys(&r.keys), &mut r.source,
                &fixture::source(&r.root), None, || {
                    calls += 1; assert_ne!(calls, panic_at, "interrupted capture-start clock"); ElapsedTick(2)
                })
        })).is_err());
        assert_eq!(r.source.status().read_attempts - reads, u64::from(panic_at == 3));
        assert!(r.host.storage_failure().is_some()); assert!(r.host.file_source_status().unwrap().interrupted);
        assert_eq!(r.host.revision(), before + 1);
        assert_eq!(r.host.inspect().executions, 0); assert_eq!(r.host.inspect().control.ledger.reserved, 16);
    }
}

#[test]
fn missing_native_profile_and_full_fixed_capacity_refuse_before_any_acquisition() {
    let mut legacy = Ready::configured(profile(), false, false); let before = legacy.host.inspect();
    let report = legacy.host.complete_publication_from_files(before.revision, keys(&legacy.keys), &mut legacy.source,
        &fixture::source(&legacy.root), None, || panic!("missing native profile before clock"));
    assert_eq!(report.completion.completion.result, Err(JournalError::Contract(Error::Incomplete)));
    assert!(report.observations.is_empty()); assert_eq!(legacy.host.inspect(), before);
    let standard = Ready::new(false); let mut small = profile();
    small.delivery.limits.events = standard.host.revision() as usize + 9;
    let mut r = Ready::configured(small, true, false); let before = r.host.inspect();
    let report = r.host.complete_publication_from_files(before.revision, keys(&r.keys), &mut r.source,
        &fixture::source(&r.root), None, || panic!("full completion admission before clock"));
    assert_eq!(report.completion.completion.result, Err(JournalError::Contract(Error::Limit)));
    assert!(report.observations.is_empty()); assert_eq!(r.host.inspect(), before);
}

#[test]
fn foreign_human_keys_cannot_trigger_a_native_source_read_or_reservation() {
    let mut r = Ready::new(false); let other = Ready::new(false); let before = r.host.inspect();
    let report = r.host.complete_publication_from_files(before.revision, CapturedCompletionKeys {
        automatic: &r.keys.automatic, human: &other.keys.human, credential: None,
    }, &mut r.source, &fixture::source(&r.root), None, || panic!("foreign human before clock"));
    assert_eq!(report.completion.completion.result, Err(JournalError::Contract(Error::Binding)));
    assert!(report.observations.is_empty()); assert_eq!(r.host.inspect(), before);
    assert_eq!(r.complete(2).completion.completion.result.unwrap().basis, PublicationBasis::Revalidated);
}
