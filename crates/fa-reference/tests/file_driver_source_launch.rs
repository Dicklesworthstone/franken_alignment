//! Durable source admission before the original socket review is started.
#![cfg(unix)]
#[path = "support/file_driver_source.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::FileSourceReviewError;
use fa_reference::action::consequence::delivery::persistent::observed::helpers::FileHelperSetupError;
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourceError;
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceError, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::policy_state::StateLimits;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::fs;
use std::io::ErrorKind;

#[test]
fn changed_launch_observation_replaces_only_its_own_checked_input_predecessor() {
    let mut file = configured(1, 3, StateLimits::default());
    {
        let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
        let previous = document(1).inputs_for(host.request_action(1).unwrap(), &driver::profile().committee).unwrap();
        let revision = host.revision(); host.record_inputs(revision, 1, 0, previous).unwrap();
    }
    let prepared = launch(&mut file, 101);
    let expected = prepared.expected_input_revision;
    assert!(expected > 0);
    replace(&file.path, &document(2));
    let count = capture_count(&file);
    // Bootstrap's three-tick lease is now expired. The new read itself renews
    // the native source and withdraws the previous different helper context.
    let observed = file.rig.driver.start_file_review(&mut file.source, prepared, || ElapsedTick(4)).unwrap();
    assert_eq!(observed, document(2).identity());
    assert_eq!(capture_count(&file), count + 1);
    {
        let host = file.rig.driver.supervisor().host().unwrap();
        assert!(host.input_revision(1).unwrap() > expected);
        assert_eq!(host.file_source_status().unwrap().producer.unwrap().generation, 2);
    }
    let review = finish(&mut file, 4, Verdict::Allow);
    assert_eq!(review.source_updates, vec![Ok(document(2).identity())]);
    assert!(matches!(review.result, Ok(FileDriverEvent::ReviewApplied { .. })));
    let key = human(&mut file, 4);
    let result = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(4), Some(&key));
    assert!(matches!(result.result, Ok(FileDriverEvent::Dispatched { .. })));
}

#[test]
fn stale_caller_revision_is_rejected_before_any_read_clock_or_native_source_change() {
    let mut file = configured(1, 10, StateLimits::default());
    let mut prepared = launch(&mut file, 101);
    prepared.expected_input_revision += 1;
    replace(&file.path, &document(2));
    let reads = file.source.status().read_attempts; let count = capture_count(&file);
    let before = file.rig.driver.supervisor().host().unwrap().inspect();
    let result = file.rig.driver.start_file_review(&mut file.source, prepared, || panic!("stale launch sampled time"));
    assert!(matches!(result, Err(FileSourceReviewError::Control(JournalError::Contract(Error::Stale)))));
    assert_eq!(file.source.status().read_attempts, reads); assert_eq!(capture_count(&file), count);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
    let fresh = launch(&mut file, 101);
    assert_eq!(file.rig.driver.start_file_review(&mut file.source, fresh, || ElapsedTick(1)).unwrap(), document(2).identity());
}

#[test]
fn launch_lost_source_withdraws_the_mandatory_gate_not_just_one_manual_snapshot() {
    let mut file = configured(1, 10, StateLimits::default()); let prepared = launch(&mut file, 101);
    fs::remove_file(&file.path).unwrap();
    let result = file.rig.driver.start_file_review(&mut file.source, prepared, || ElapsedTick(1));
    assert!(matches!(result, Err(FileSourceReviewError::DurableSource(FileSourceError::Read {
        error: EvidenceError::Io(ErrorKind::NotFound), withdrawal: None,
    }))));
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
    {
        let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
        assert_eq!(host.file_source_status().unwrap().capture.closed, None);
        let revision = host.revision(); let spec = driver::helper::spec(&host, b"separate request");
        assert_eq!(host.propose(revision, 2, spec, document(1).snapshot().clone()).unwrap_err(),
            JournalError::Contract(Error::Incomplete));
        assert_eq!(host.inspect().executions, 0);
    }
    // No native review began during the failed launch. A complete new read can
    // start this round, without treating the previous outage as a completed vote.
    replace(&file.path, &document(1)); let prepared = launch(&mut file, 101);
    file.rig.driver.start_file_review(&mut file.source, prepared, || ElapsedTick(1)).unwrap();
    assert!(matches!(finish(&mut file, 1, Verdict::Allow).result, Ok(FileDriverEvent::ReviewApplied { .. })));
}

#[test]
fn a_new_reader_below_the_durable_floor_cannot_even_start_a_helper_round() {
    let mut file = configured(2, 10, StateLimits::default());
    replace(&file.path, &document(1));
    file.source = FileEvidenceSource::new(&file.path, SOURCE, driver::profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    let prepared = launch(&mut file, 101);
    let result = file.rig.driver.start_file_review(&mut file.source, prepared, || ElapsedTick(1));
    assert!(matches!(result, Err(FileSourceReviewError::DurableSource(FileSourceError::Refused(Error::Stale)))));
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
    let host = file.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.file_source_status().unwrap().producer.unwrap().generation, 2);
    assert_eq!(host.file_source_status().unwrap().capture.closed, None);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn read_completion_at_lease_expiry_refuses_launch_without_burning_the_round() {
    for finish_at in [4, 5] {
        let mut file = configured(1, 3, StateLimits::default());
        let prepared = launch(&mut file, 101); let count = capture_count(&file); let mut calls = 0;
        let result = file.rig.driver.start_file_review(&mut file.source, prepared, || {
            calls += 1;
            ElapsedTick(if calls == 1 { 2 } else { finish_at })
        });
        assert_eq!(capture_count(&file), count + 1);
        if finish_at == 4 { assert!(result.is_ok()); }
        else {
            assert!(matches!(result, Err(FileSourceReviewError::Sockets {
                error: FileHelperSetupError::Journal(JournalError::Contract(Error::Stale)), ..
            })));
            assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
            let prepared = launch(&mut file, 101);
            file.rig.driver.start_file_review(&mut file.source, prepared, || ElapsedTick(5)).unwrap();
        }
        assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn source_capture_quota_is_consumed_by_launch_and_cannot_be_skipped_to_obtain_workers() {
    for budget in [1, 2] {
        let mut file = configured(1, 10, StateLimits { events: budget, ..StateLimits::default() });
        let prepared = launch(&mut file, 101);
        let result = file.rig.driver.start_file_review(&mut file.source, prepared, || ElapsedTick(1));
        if budget == 2 {
            assert!(result.is_ok()); assert_eq!(capture_count(&file), 2);
        } else {
            assert!(matches!(result, Err(FileSourceReviewError::DurableSource(FileSourceError::Refused(Error::Limit)))));
            assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
            let status = file.rig.driver.supervisor().host().unwrap().file_source_status().unwrap();
            assert_eq!(status.capture.fault, Some(Error::Limit)); assert_eq!(status.capture.closed, None);
        }
        assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    }
}
