//! Real numerical journals and native review; settlement is not an effect retry.
//! Synthetic models/probabilities test orchestration, not calibration.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::FileLearnedRecoveryStatus,
    guarded::{FileGuardSet, FileRecoveryRequirements,
        predictive::{FilePredictiveRequirements, owned::FileOwnedPredictiveRequirements}},
};
use fa_reference::action::consequence::oversight::actor::UnknownReason;
use std::{cell::Cell, collections::BTreeMap, path::Path};

pub(super) fn files(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(path).unwrap().map(|entry| {
        let path = entry.unwrap().path();
        (path.clone(), fs::read(path).unwrap())
    }).collect()
}

// Exercise the actual public guarded methods; no private candidate or fabricated
// outcome enters these tests. The callback selects canonical-change fault cuts.
fn reconcile(config: &Config, loaded: &Loaded, selection: (u64, Option<u64>, bool),
    before_finish: impl FnOnce(), clock: impl FnOnce() -> ElapsedTick)
    -> Result<Knowledge<ActorOutcome>, JournalError>
{
    let (request, after, replay) = selection;
    let expected = FileRecoveryRequirements {
        guards: FileGuardSet { stream: Some(loaded.stream), decoder: None, decoder_stop: None,
            source: Some(config.source_policy), identity: None, campaigns: None, credential: None },
        effective_policy: config.profile.delivery.policy.clone(), credential_epoch: None,
        minimum: loaded.floor,
    };
    let anchor = loaded.recovery_anchor.as_ref().unwrap();
    macro_rules! finish {
        ($opened:expr) => {{
            let mut run = $opened?.require_recovery_reserve(RecoveryReserve::terminal())?;
            if replay {
                while run.progress().status == FileLearnedRecoveryStatus::Replaying {
                    run.advance(run.progress().replayed_events, 3)?;
                }
                assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
            }
            before_finish();
            run.finish_reconciled_receipt(request, after, clock)
        }};
    }
    if let Some(prediction) = loaded.generation.required_pre_output_forecast() {
        let expected = FilePredictiveRequirements { oversight: expected,
            prediction: prediction.clone(), evaluation: None };
        finish!(FileOversight::begin_open_predictive_guarded_anchored_with_learned_generation(
            &config.store, config.profile.clone(), &expected, &loaded.generation, anchor))
    } else if let Some(prediction) = loaded.generation.required_owned_pre_output_forecast() {
        let expected = FileOwnedPredictiveRequirements { oversight: expected,
            prediction: prediction.clone(), evaluation: None };
        finish!(FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
            &config.store, config.profile.clone(), &expected, &loaded.generation, anchor))
    } else {
        finish!(FileOversight::begin_open_guarded_anchored_with_learned_generation(
            &config.store, config.profile.clone(), &expected, &loaded.generation, anchor))
    }
}

pub(super) struct Sent {
    pub config: Config,
    pub path: PathBuf,
    pub loaded: Loaded,
    pub revision: u64,
    pub charged: u64,
}

pub(super) fn sent(root: &Root, mode: Option<bool>, published: bool) -> Sent {
    let (mut config, path, mut loaded, reviewer, mut driver, mut wire, deadline) = generated(root, mode);
    submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
    let (returned, history) = native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
    driver = returned;
    let human = native::approve(&mut driver, &mut config, &reviewer, 1);
    assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), Some(&human)).result.unwrap(), FileDriverEvent::Dispatched { .. }));
    if published {
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::PublicationChecked { .. }));
    }
    assert!(matches!(response(&mut wire, 1).result,
        Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown })));
    let host = driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, u64::from(published));
    assert!(host.request_resolution(1).unwrap().is_none());
    loaded.recovery_anchor = Some(host.history_anchor().unwrap());
    let revision = host.revision(); let charged = host.inspect().control.ledger.charged;
    assert!(charged > 0); drop(host);
    assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
    Sent { config, path, loaded, revision, charged }
}

#[test]
fn one_replayed_settlement_matches_original_three_transitions_byte_for_byte() {
    for mode in [None, Some(false), Some(true)] {
        for published in [false, true] {
            let root = Root::new(); let mut case = sent(&root, mode, published);
            remove_live_assets(&root);
            let anchor = case.loaded.recovery_anchor.take();
            case.loaded = recipe::load(&case.path, &case.config, false).unwrap();
            case.loaded.recovery_anchor = anchor;
            let canonical = case.config.store.join("delivery.bin");
            let original = fs::read(&canonical).unwrap();
            // Independent oracle: original public Fence, Time, and Sweep on
            // this test store, then return the fixture to its pre-settlement cut.
            let (mut control, _, _) = recovery::open_selected(&case.config, &case.loaded).unwrap();
            let numerical = control.learned_generation_inspection().unwrap().numerical;
            let evidence = mode.map(|_| control.action_consistency_snapshot().unwrap());
            let owned = (mode == Some(true)).then(|| control.learned_action_consistency_snapshot().unwrap());
            control.observe_time(control.revision(), ElapsedTick(1001)).unwrap();
            control.reconcile_pending(control.revision()).unwrap();
            assert_eq!(control.revision(), case.revision + 3);
            assert_eq!(control.inspect().executions, u64::from(published));
            assert_eq!(control.inspect().control.ledger.charged, case.charged);
            assert_eq!(control.learned_generation_inspection().unwrap().numerical, numerical);
            if let Some(evidence) = evidence { assert_eq!(control.action_consistency_snapshot().unwrap().evidence, evidence.evidence); }
            if let Some(owned) = owned { assert_eq!(control.learned_action_consistency_snapshot().unwrap().work, owned.work); }
            let deadline = control.learned_text_stream_message_request(1).unwrap().deadline;
            let (port, supervisor) = control.into_learned_text_stream_actor_gateway().unwrap();
            let mut wire = ActorWire::new(port);
            let proposal = FileLearnedTextStreamActorPort::encode_release(1,
                LearnedTextRelease::Message, deadline).unwrap();
            let expected = wire.exchange(&encode_command(&Command::Submit { request: 1, proposal }).unwrap()).result.unwrap();
            assert_eq!(supervisor.host().unwrap().revision(), case.revision + 3);
            assert_eq!(matches!(&expected, Knowledge::Known { value: ActorOutcome::Executed, .. }), published);
            if !published { assert!(matches!(&expected, Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown })); }
            let expected_bytes = fs::read(&canonical).unwrap(); drop(wire); drop(supervisor);
            fs::write(&canonical, &original).unwrap();
            fs::write(case.config.store.join("delivery.pending"), b"pre-settlement staging").unwrap();
            let calls = Cell::new(0);
            let result = reconcile(&case.config, &case.loaded, (1, None, true), || {}, || {
                calls.set(calls.get() + 1); ElapsedTick(1001)
            }).unwrap();
            assert_eq!(calls.get(), 1); assert_eq!(result, expected);
            assert_eq!(fs::read(&canonical).unwrap(), expected_bytes);
            assert!(!case.config.store.join("delivery.pending").exists());
            // The committed result can subsequently be read without another
            // fence, clock or reconciliation. Unknown is never upgraded by reading.
            let before_read = files(&case.config.store);
            let read = recovery::readonly::read(&case.config, &case.loaded, None, &mut || Duration::ZERO).unwrap();
            assert_eq!(read.result.unwrap(), expected); assert_eq!(files(&case.config.store), before_read);
        }
    }
}

fn denied(root: &Root, mode: Option<bool>) -> (Config, Loaded) {
    let (mut config, _, mut loaded, _, mut driver, mut wire, deadline) = generated(root, mode);
    evidence(root, &config, 2, false, false);
    submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
    assert!(matches!(response(&mut wire, 1).result, Ok(Knowledge::Known { value: ActorOutcome::NotAdmitted, .. })));
    loaded.recovery_anchor = Some(driver.supervisor().host().unwrap().history_anchor().unwrap());
    assert_eq!(cleanup(driver, 1000, 1), 0);
    fs::write(config.store.join("delivery.pending"), b"preserve until commit").unwrap();
    (config, loaded)
}

#[test]
fn guarded_settlement_refuses_bad_selection_incomplete_replay_and_floor_before_clock() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let (config, mut loaded) = denied(&root, mode);
        let original = files(&config.store);
        for selection in [(1, None, false), (0, None, true), (2, None, true), (1, Some(1), true), (1, Some(9), true)] {
            assert!(reconcile(&config, &loaded, selection, || {}, || panic!("invalid selection reached clock")).is_err());
            assert_eq!(files(&config.store), original);
        }
        let floor = loaded.floor;
        loaded.floor.journal_revision = loaded.recovery_anchor.as_ref().unwrap().revision() + 1;
        assert!(reconcile(&config, &loaded, (1, None, true), || {}, || panic!("unmet floor reached clock")).is_err());
        loaded.floor = floor; assert_eq!(files(&config.store), original);
        let result = reconcile(&config, &loaded, (1, None, true), || {}, || ElapsedTick(1001)).unwrap();
        assert!(matches!(result, Knowledge::Known { value: ActorOutcome::NotAdmitted, .. }));
    }
}

#[test]
fn canonical_changes_and_unwinding_clock_never_publish_private_settlement() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let (config, loaded) = denied(&root, mode);
        let canonical = config.store.join("delivery.bin"); let original = files(&config.store);
        let changed = b"changed canonical cut";
        assert_eq!(reconcile(&config, &loaded, (1, None, true), || {
            fs::write(&canonical, changed).unwrap();
        }, || panic!("pre-clock canonical mismatch must refuse")).err(), Some(Error::Binding.into()));
        assert_eq!(fs::read(&canonical).unwrap(), changed);
        fs::write(&canonical, &original[&canonical]).unwrap();
        let calls = Cell::new(0);
        assert_eq!(reconcile(&config, &loaded, (1, None, true), || {}, || {
            calls.set(calls.get() + 1); fs::write(&canonical, changed).unwrap(); ElapsedTick(1001)
        }).err(), Some(Error::Binding.into()));
        assert_eq!(calls.get(), 1); assert_eq!(fs::read(&canonical).unwrap(), changed);
        assert_eq!(fs::read(config.store.join("delivery.pending")).unwrap(), b"preserve until commit");
        fs::write(&canonical, &original[&canonical]).unwrap();
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            reconcile(&config, &loaded, (1, None, true), || {}, || panic!("clock interrupted"))
        }));
        assert!(interrupted.is_err()); assert_eq!(files(&config.store), original);
        // A successful new cursor proves both that no lock/candidate escaped and
        // that the exact old anchor still accepts the unchanged source history.
        assert!(matches!(reconcile(&config, &loaded, (1, None, true), || {}, || ElapsedTick(1001)).unwrap(),
            Knowledge::Known { value: ActorOutcome::NotAdmitted, .. }));
    }
}

#[test]
fn settlement_never_spends_terminal_only_capacity_but_readonly_receipts_still_work() {
    for mode in [None, Some(false), Some(true)] {
        for full in [false, true] {
            let root = Root::new(); let mut bounded = configured(&root);
            bounded.profile.delivery.limits.events = 256;
            let (mut config, _, mut loaded, _, mut driver, mut wire, deadline) = generated_with_config(&root, mode, bounded);
            evidence(&root, &config, 2, false, false);
            submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
            {
                let mut host = driver.supervisor_mut().host_mut().unwrap();
                while host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
                    let revision = host.revision(); host.observe_time(revision, ElapsedTick(1000)).unwrap();
                }
                if full {
                    while host.revision() < 256 { let revision = host.revision(); host.fence(revision).unwrap(); }
                }
                loaded.recovery_anchor = Some(host.history_anchor().unwrap());
            }
            let expected = response(&mut wire, 1).result.unwrap();
            assert_eq!(cleanup(driver, 1000, 1), 0);
            fs::write(config.store.join("delivery.pending"), b"capacity refusal staging").unwrap();
            let before = files(&config.store); let calls = Cell::new(0);
            assert_eq!(reconcile(&config, &loaded, (1, None, true), || {}, || {
                calls.set(calls.get() + 1); ElapsedTick(1001)
            }).err(), Some(Error::Limit.into()));
            if full { assert_eq!(calls.get(), 0); }
            assert_eq!(files(&config.store), before);
            let read = recovery::readonly::read(&config, &loaded, None, &mut || Duration::ZERO).unwrap();
            assert_eq!(read.result.unwrap(), expected); assert_eq!(files(&config.store), before);
        }
    }
}

#[path = "reconciled/operator.rs"] mod operator;
