//! Original source/probe/wire paths; no byte decoder may invent certified bounds.
#[path = "support/sidecar_receiving.rs"]
pub mod shared;
use shared::*;
use fa_reference::Error;
use fa_reference::action::consequence::activation::probe::{ProbeOutcome, learned::{KvRefinementBudget, LearnedProbeWork}};
use fa_reference::action::consequence::oversight::{helper_workers::wire::{decode_request, encode_request},
    sidecar::receiver::{SidecarReceiver, SidecarReceiveBudget}};
use fa_reference::round::Verdict;

#[test]
fn coarse_receiver_uses_original_intervals_without_undisclosed_exact_residuals() {
    let mut setup = Setup::new(b"approve?"); let (_round, ports) = setup.workers(10);
    let received = SidecarReceiver::new(&ports["reviewer"], &setup.packet, setup.source.clone(),
        SidecarReceiveBudget::default()).unwrap().receive(&input(&ports["reviewer"])).unwrap();
    assert_eq!(received.disclosed_groups().count(), 0);
    assert_eq!(received.work().materialized_values, 0);
    assert!(setup.source.residual_bytes(group()).is_ok());
    let probe = probe(0, 0.5);
    let planned = received.probe_work(&probe, row()).unwrap();
    let actual = received.evaluate_probe(&probe, row(), planned).unwrap();
    let independent = probe.evaluate_learned(&setup.source.view(), row()).unwrap();
    assert_eq!(actual.interval(), independent.interval());
    assert_eq!(actual.outcome(), ProbeOutcome::NeedsRefinement);
    assert_eq!(actual.work(), independent.work());
    assert_eq!(actual.frame(), independent.frame());
    assert_eq!(actual.probe(), independent.probe());
}

#[test]
fn disclosed_refinement_changes_the_actual_score_not_just_a_residual_count() {
    let mut setup = Setup::new(b"approve?"); let (mut round, ports) = setup.workers(10);
    let original = input(&ports["reviewer"]);
    let coarse = SidecarReceiver::new(&ports["reviewer"], &setup.packet, setup.source.clone(),
        SidecarReceiveBudget::default()).unwrap().receive(&original).unwrap();
    let probe = probe(0, 0.5);
    let observation = coarse.evaluate_probe(&probe, row(), coarse.probe_work(&probe, row()).unwrap()).unwrap();
    assert_eq!(observation.outcome(), ProbeOutcome::NeedsRefinement);
    let review = finish(&mut round, &ports["reviewer"], Verdict::Abstain); setup.refine(&review);
    let (_next, ports) = setup.workers(11);
    let received = SidecarReceiver::new(&ports["reviewer"], &setup.packet, setup.source.clone(),
        SidecarReceiveBudget::default()).unwrap().receive(&input(&ports["reviewer"])).unwrap();
    assert_eq!(received.disclosed_groups().collect::<Vec<_>>(), vec![group()]);
    let actual = received.evaluate_probe(&probe, row(), received.probe_work(&probe, row()).unwrap()).unwrap();
    let mut independent = setup.source.view();
    let block = setup.source.verify_residual(group(), setup.source.residual_bytes(group()).unwrap()).unwrap();
    let work = independent.refine(0, &block, KvRefinementBudget::default()).unwrap();
    let expected = probe.evaluate_learned(&independent, row()).unwrap();
    assert_eq!(actual.interval(), expected.interval());
    assert_eq!(actual.outcome(), ProbeOutcome::CertifiedQuiet);
    assert_eq!(actual.interval().lower, actual.interval().upper);
    assert_eq!(actual.work().reconstruction_products, 0);
    assert_eq!(received.work().materialized_values, work.materialized_values);
    assert_eq!(received.work().reconstruction_products, work.reconstruction_products);
    // A different direction on the same truly disclosed words is an alarm.
    let alarm = shared::probe(1, 0.5);
    assert_eq!(received.evaluate_probe(&alarm, row(), received.probe_work(&alarm, row()).unwrap())
        .unwrap().outcome(), ProbeOutcome::CertifiedAlarm);
    // The earlier view remains coarse, despite sharing its source with this one.
    assert_eq!(coarse.evaluate_probe(&probe, row(), coarse.probe_work(&probe, row()).unwrap())
        .unwrap().outcome(), ProbeOutcome::NeedsRefinement);
}

#[test]
fn exact_request_binding_checks_root_round_member_limits_question_and_payload() {
    let mut setup = Setup::new(b"approve?"); let (_round, ports) = setup.workers(10);
    let port = &ports["reviewer"]; let frame = encode_request(port).unwrap();
    for field in 0..8 {
        let receiver = SidecarReceiver::new(port, &setup.packet, setup.source.clone(), SidecarReceiveBudget::default()).unwrap();
        let mut altered = frame.clone();
        match field {
            0 => altered[16] ^= 1, // round, same legal framing
            1 => altered[17] ^= 1, // root, not compared via the FNV digest
            2 => altered[51] = b's', // same-length valid UTF-8 member
            3 => altered[59..61].copy_from_slice(&128_u16.to_be_bytes()),
            4 => altered[68] ^= 1, // original input profile ID
            _ => {
                let needle = match field { 5 => b"approve?".as_slice(), 6 => b"visible", _ => b"FASIDE" };
                let offset = altered.windows(needle.len()).position(|part| part == needle).unwrap();
                altered[offset] ^= 1;
            }
        }
        let parsed = decode_request(&altered).unwrap();
        assert!(matches!(receiver.receive(&parsed), Err(Error::Binding)), "field {field}");
    }
    assert!(SidecarReceiver::new(port, &setup.packet, setup.source.clone(), SidecarReceiveBudget::default())
        .unwrap().receive(&decode_request(&frame).unwrap()).is_ok());
}

#[test]
fn matching_shapes_cannot_replace_the_source_or_a_different_members_actual_view() {
    let mut setup = Setup::new(b"approve?"); let (_round, ports) = setup.workers(10);
    assert!(matches!(SidecarReceiver::new(&ports["reviewer"], &setup.packet, source(22),
        SidecarReceiveBudget::default()), Err(Error::Binding)));
    let different = Setup::new(b"another question?");
    assert!(matches!(SidecarReceiver::new(&ports["reviewer"], &different.packet, different.source,
        SidecarReceiveBudget::default()), Err(Error::Binding)));
    assert!(SidecarReceiver::new(&ports["reviewer"], &setup.packet, setup.source.clone(),
        SidecarReceiveBudget::default()).is_ok());
}

#[test]
fn all_receiver_allowances_cover_the_whole_selected_disclosure_before_materialization() {
    let mut setup = refined(); let (_round, ports) = setup.workers(11); let port = &ports["reviewer"];
    let work = SidecarReceiver::new(port, &setup.packet, setup.source.clone(), SidecarReceiveBudget::default())
        .unwrap().admitted_work();
    assert!(work.residual_bytes > 0 && work.materialized_values > 0 && work.reconstruction_products > 0);
    let exact = SidecarReceiveBudget { payload_bytes: work.payload_bytes, request_bytes: work.request_bytes,
        residual: KvRefinementBudget { encoded_bytes: work.residual_bytes, materialized_values: work.materialized_values,
            reconstruction_products: work.reconstruction_products } };
    let received = SidecarReceiver::new(port, &setup.packet, setup.source.clone(), exact).unwrap().receive(&input(port)).unwrap();
    assert_eq!(received.work(), work);
    for field in 0..5 {
        let mut short = exact;
        match field {
            0 => short.payload_bytes -= 1, 1 => short.request_bytes -= 1,
            2 => short.residual.encoded_bytes -= 1, 3 => short.residual.materialized_values -= 1,
            _ => short.residual.reconstruction_products -= 1,
        }
        expect_limit(SidecarReceiver::new(port, &setup.packet, setup.source.clone(), short));
    }
}

#[test]
fn probe_admission_uses_original_cost_and_rejects_foreign_rows_without_partial_scores() {
    let mut setup = Setup::new(b"approve?"); let (_round, ports) = setup.workers(10);
    let received = SidecarReceiver::new(&ports["reviewer"], &setup.packet, setup.source.clone(),
        SidecarReceiveBudget::default()).unwrap().receive(&input(&ports["reviewer"])).unwrap();
    let probe = probe(0, 0.5); let work = received.probe_work(&probe, row()).unwrap();
    assert!(work.coordinates > 0 && work.reconstruction_products > 0);
    expect_limit(received.evaluate_probe(&probe, row(), LearnedProbeWork { coordinates: work.coordinates - 1, ..work }));
    expect_limit(received.evaluate_probe(&probe, row(), LearnedProbeWork { reconstruction_products: work.reconstruction_products - 1, ..work }));
    let mut foreign = row(); foreign.position = 1;
    assert!(matches!(received.evaluate_probe(&probe, foreign, work), Err(Error::Missing)));
    assert_eq!(received.evaluate_probe(&probe, row(), work).unwrap().outcome(), ProbeOutcome::NeedsRefinement);
}

#[test]
fn a_new_round_cannot_receive_a_previous_rounds_identical_coarse_request() {
    let mut setup = Setup::new(b"approve?"); let (mut round, ports) = setup.workers(10);
    let previous = input(&ports["reviewer"]); let _review = finish(&mut round, &ports["reviewer"], Verdict::Abstain);
    let (_round, ports) = setup.workers(11);
    let current = input(&ports["reviewer"]);
    assert_eq!(previous.actual_input(), current.actual_input());
    let receiver = SidecarReceiver::new(&ports["reviewer"], &setup.packet, setup.source.clone(), SidecarReceiveBudget::default()).unwrap();
    assert!(matches!(receiver.receive(&previous), Err(Error::Binding)));
    assert!(SidecarReceiver::new(&ports["reviewer"], &setup.packet, setup.source.clone(), SidecarReceiveBudget::default())
        .unwrap().receive(&current).is_ok());
}
