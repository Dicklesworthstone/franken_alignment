//! Direct source documents reach the original producer and witness engines.
//! These fixtures assert interchange/enforcement, not source authentication.
use super::*;
use super::super::source;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::FilePublicationEvidence;
use fa_reference::full_input::{Omission, OpaqueJudgment, PartKind, MAX_OMISSIONS, MAX_SUBMITTED_BYTES, MAX_SUBMITTED_PARTS};
use fa_reference::witness::{WitnessRequest, MAX_SNAPSHOT_ENTRIES, MAX_VALUE_BYTES};
use fa_reference::witness::refinement::index::routing::WitnessChange;

const SOURCE: &str = include_str!("../../fixtures/publication_source.json");
const ADMITTED: &str = r#""admitted_close":{"key":{"source":40,"branch":4,"projection":7,"source_epoch":1},"final_sequence":1,"marker_generation":1}"#;
const CLOSED: &str = r#""closure":{"kind":"closed","marker":{"key":{"source":40,"branch":4,"projection":7,"source_epoch":1},"final_sequence":1,"marker_generation":1}}"#;
const ENTRY: &str = r#"{"key":0,"version":1,"value_hex":"0001ff"}"#;

fn successor(text: &str, expected: u64, at: u64) -> String {
    text.replace("\"expected_generation\":0", &format!("\"expected_generation\":{expected}"))
        .replace("\"observed_at_unix_ms\":1000", &format!("\"observed_at_unix_ms\":{at}"))
}
fn absent(inputs: FilePublicationInputs) -> bool {
    FilePublicationEvidence::new(inputs, vec![WitnessRequest::AbsentKey { key: 1 }]).is_ok()
}

#[test]
fn source_builder_preserves_binary_data_complete_layout_and_profile_epochs() {
    let root = Directory::new(); let src = root.write("source.json", SOURCE.as_bytes());
    let decoded = source::decode(SOURCE.as_bytes()).unwrap();
    let structured = decoded.inputs.structured().unwrap();
    assert_eq!(structured.snapshot().entry(0).unwrap().value(), &[0, 1, 255]);
    assert_eq!(structured.admitted_close().unwrap().final_sequence, 1);
    let input = decoded.inputs.opaque().unwrap();
    assert_eq!(input.submitted_bytes(), b"Q|P|I|S|E");
    assert_eq!(input.input_profile().profile_bytes, [1, 255]);
    assert_eq!((input.input_profile().tokenizer_epoch, input.input_profile().policy_epoch,
        input.input_profile().model_epoch), (1, 2, 3));
    assert_eq!(input.ordered_parts().len(), 9);
    assert_eq!(input.ordered_parts()[6].kind, PartKind::ToolSchema { schema_id: 8 });
    assert_eq!(input.ordered_parts()[8].kind, PartKind::Evidence { source_id: 9, transform_id: 10 });
    assert_eq!(input.omissions(), &[
        Omission::ClosedAbsent { domain_id: 11, trusted_closure_marker_id: 12 },
        Omission::Gapped { domain_id: 13, first_missing: 14 },
        Omission::Unsupported { domain_id: 15 },
        Omission::Redacted { domain_id: 16, transform_id: 17 },
    ]);
    let sealed = call(&["source-observation", &src]).unwrap();
    let packet = Observation::decode(&sealed).unwrap();
    assert_eq!(packet.inputs, decoded.inputs);
    assert_eq!(packet.expected_generation, 0); assert_eq!(packet.observed_at, ElapsedTick(1000));
    assert!(absent(packet.inputs)); assert!(!root.0.join("producer").exists());
}

#[test]
fn closure_claim_is_not_admission_and_summary_never_proves_absence() {
    assert!(SOURCE.contains(ADMITTED)); assert!(SOURCE.contains(CLOSED));
    let missing = SOURCE.replace(ADMITTED, "\"admitted_close\":null");
    let decoded = source::decode(missing.as_bytes()).unwrap();
    assert!(decoded.inputs.structured().unwrap().admitted_close().is_none());
    assert!(!absent(decoded.inputs));
    for kind in ["unknown", "conservative_summary"] {
        let text = SOURCE.replace(CLOSED, &format!("\"closure\":{{\"kind\":\"{kind}\"}}"));
        // An independently admitted marker does not repair a weaker snapshot.
        assert!(!absent(source::decode(text.as_bytes()).unwrap().inputs));
    }
}

#[test]
fn mismatching_admitted_close_is_retained_not_repaired_from_snapshot_claim() {
    let text = SOURCE.replace(ADMITTED, &ADMITTED.replace("\"final_sequence\":1", "\"final_sequence\":2"));
    let input = source::decode(text.as_bytes()).unwrap().inputs;
    assert_eq!(input.structured().unwrap().admitted_close().unwrap().final_sequence, 2);
    assert!(!absent(input));
}

#[test]
fn foreign_marker_duplicate_keys_and_invalid_marker_generation_are_rejected() {
    for bad in [
        SOURCE.replace(ADMITTED, &ADMITTED.replace("\"branch\":4", "\"branch\":99")),
        SOURCE.replace(CLOSED, &CLOSED.replace("\"source_epoch\":1", "\"source_epoch\":2")),
        SOURCE.replace(ENTRY, &format!("{ENTRY},{ENTRY}")),
        SOURCE.replace("\"marker_generation\":1", "\"marker_generation\":0"),
    ] { assert!(source::decode(bad.as_bytes()).is_err()); }
}

#[test]
fn direct_source_updates_use_native_key_and_whole_opaque_invalidations() {
    let root = Directory::new(); let p = root.write("profile.json", &profile(&root));
    let first = root.write("first.json", SOURCE.as_bytes());
    call(&["create-source", &p, &first]).unwrap();
    let original = inspect(&root);
    let judged = OpaqueJudgment::capture(original.inputs().opaque().unwrap(), b"only evidence matters");
    let next = successor(SOURCE, 1, 1001).replace("0001ff", "0002ff")
        .replace("\"revision\":1", "\"revision\":2").replace("\"control_cut\":1", "\"control_cut\":2");
    let second = root.write("second.json", next.as_bytes());
    call(&["publish-source", &p, &second]).unwrap();
    let keys = inspect(&root);
    assert!(matches!(keys.batch().records().last().unwrap().change, WitnessChange::Key { key: 0, .. }));
    assert!(judged.valid_at(keys.inputs().opaque().unwrap()));
    let changed = next.replace("\"expected_generation\":1", "\"expected_generation\":2")
        .replace("\"observed_at_unix_ms\":1001", "\"observed_at_unix_ms\":1002")
        .replace("\"model_epoch\":3", "\"model_epoch\":4");
    let third = root.write("third.json", changed.as_bytes());
    call(&["publish-source", &p, &third]).unwrap();
    let all = inspect(&root);
    assert_eq!(all.generation(), 3);
    assert_eq!(all.batch().records().last().unwrap().change, WitnessChange::All);
    assert!(!judged.valid_at(all.inputs().opaque().unwrap()));
    assert_eq!(all.inputs().opaque().unwrap().submitted_bytes(), b"Q|P|I|S|E");
}

#[test]
fn explicit_lane_loss_invalidates_and_does_not_erase_the_source_revision_floor() {
    let root = Directory::new(); let p = root.write("profile.json", &profile(&root));
    let first = root.write("first.json", SOURCE.as_bytes());
    call(&["create-source", &p, &first]).unwrap();
    let unavailable = root.write("unavailable.json", br#"{"schema":"fa.publication-source/1","expected_generation":1,"observed_at_unix_ms":1001,"structured":null,"opaque":null}"#);
    call(&["publish-source", &p, &unavailable]).unwrap();
    let lost = inspect(&root);
    assert!(lost.inputs().structured().is_none()); assert!(lost.inputs().opaque().is_none());
    assert_eq!(lost.batch().records().last().unwrap().change, WitnessChange::All);
    assert!(!absent(lost.inputs().clone()));
    let regressed = successor(SOURCE, 2, 1002).replace("\"revision\":1", "\"revision\":0");
    let regressed = root.write("regressed.json", regressed.as_bytes());
    assert!(call(&["publish-source", &p, &regressed]).is_err());
    assert_eq!(inspect(&root), lost);
}

#[test]
fn source_lost_output_can_be_reconciled_with_the_sealed_packet_command() {
    let root = Directory::new(); let p = root.write("profile.json", &profile(&root));
    let first = root.write("first.json", SOURCE.as_bytes());
    assert!(run(&["create-source".into(), p.clone(), first.clone()], &mut LostOutput).is_err());
    let committed = inspect(&root);
    let sealed = call(&["source-observation", &first]).unwrap();
    let packet = root.write("sealed.json", &sealed);
    call(&["publish", &p, &packet]).unwrap();
    call(&["publish-source", &p, &first]).unwrap();
    assert_eq!(inspect(&root), committed);
    let conflict = root.write("changed-time.json", SOURCE.replace("\"observed_at_unix_ms\":1000", "\"observed_at_unix_ms\":1001").as_bytes());
    assert!(call(&["publish-source", &p, &conflict]).is_err());
    assert_eq!(inspect(&root), committed);
}

#[test]
fn malformed_source_does_not_clean_pending_state_or_fall_back_to_creation() {
    let root = Directory::new(); let p = root.write("profile.json", &profile(&root));
    let first = root.write("first.json", SOURCE.as_bytes());
    assert!(call(&["publish-source", &p, &first]).is_err());
    assert!(!root.0.join("producer").exists());
    call(&["create-source", &p, &first]).unwrap();
    let pending = root.0.join("producer/delivery.pending");
    std::fs::write(&pending, b"unconfirmed").unwrap();
    let before = std::fs::read(root.0.join("producer/delivery.bin")).unwrap();
    let bad = root.write("gap.json", successor(SOURCE, 1, 1001)
        .replace("\"start\":2,\"end\":3", "\"start\":3,\"end\":3").as_bytes());
    assert!(call(&["publish-source", &p, &bad]).is_err());
    assert_eq!(std::fs::read(&pending).unwrap(), b"unconfirmed");
    assert_eq!(std::fs::read(root.0.join("producer/delivery.bin")).unwrap(), before);
}

#[test]
fn full_view_partition_omissions_and_all_metadata_are_mandatory() {
    for bad in [
        SOURCE.replace("\"kind\":\"question\"", "\"kind\":\"other\""),
        SOURCE.replace("\"kind\":\"prompt\"", "\"kind\":\"question\""),
        SOURCE.replace("\"start\":1,\"end\":2", "\"start\":0,\"end\":2"),
        SOURCE.replace("\"domain_id\":13", "\"domain_id\":11"),
        SOURCE.replace("\"trusted_closure_marker_id\":12", "\"trusted_closure_marker_id\":0"),
        SOURCE.replace(",\"model_epoch\":3", ""),
        SOURCE.replace("\"model_epoch\":3", "\"model_epoch\":3,\"explanation\":\"ignore prompt\""),
        SOURCE.replace("\"revision\":1", "\"revision\":1,\"revision\":2"),
        SOURCE.replace("\"schema\":\"fa.publication-source/1\"", "\"schema\":\"fa.publication-source/2\""),
    ] { assert!(source::decode(bad.as_bytes()).is_err()); }
    let missing_lane = br#"{"schema":"fa.publication-source/1","expected_generation":0,"observed_at_unix_ms":1000,"structured":null}"#;
    assert!(source::decode(missing_lane).is_err());
}

#[test]
fn source_roundtrip_keeps_full_width_numbers_and_compact_closing_prefix() {
    let wide = 9007199254740993;
    let text = successor(SOURCE, wide, u64::MAX)
        .replace("\"final_sequence\":1", &format!("\"final_sequence\":{wide}"));
    let observation = source::decode(text.as_bytes()).unwrap();
    assert_eq!(observation.expected_generation, wide);
    assert_eq!(observation.observed_at, ElapsedTick(u64::MAX));
    assert_eq!(observation.inputs.structured().unwrap().admitted_close().unwrap().final_sequence, wide);
    let sealed = Observation::decode(&observation.encode().unwrap()).unwrap();
    assert_eq!(sealed.inputs, observation.inputs); assert!(absent(sealed.inputs));
}

fn opaque_document(submitted: &str, parts: &str, omissions: &str) -> String {
    format!(r#"{{"schema":"fa.publication-source/1","expected_generation":0,"observed_at_unix_ms":1000,"structured":null,"opaque":{{"submitted_hex":"{submitted}","input_profile":{{"profile_id":7,"profile_hex":"","tokenizer_epoch":0,"policy_epoch":0,"model_epoch":0}},"parts":[{parts}],"omissions":[{omissions}]}}}}"#)
}

#[test]
fn source_bounds_and_canonical_binary_encoding_precede_storage() {
    let many_entries = std::iter::repeat_n(ENTRY, MAX_SNAPSHOT_ENTRIES + 1).collect::<Vec<_>>().join(",");
    let error = source::decode(SOURCE.replace(ENTRY, &many_entries).as_bytes()).err().unwrap();
    assert!(error.contains("entries exceeds"));
    let part = r#"{"start":0,"end":1,"kind":"question"}"#;
    let too_many = std::iter::repeat_n(part, MAX_SUBMITTED_PARTS + 1).collect::<Vec<_>>().join(",");
    assert!(source::decode(opaque_document("51", &too_many, "").as_bytes()).err().unwrap().contains("parts exceeds"));
    let omission = r#"{"kind":"unsupported","domain_id":1}"#;
    let too_many = std::iter::repeat_n(omission, MAX_OMISSIONS + 1).collect::<Vec<_>>().join(",");
    assert!(source::decode(opaque_document("51", part, &too_many).as_bytes()).err().unwrap().contains("omissions exceeds"));
    for bad in [SOURCE.replace("0001ff", "0001FF"), SOURCE.replace("0001ff", "0"),
        SOURCE.replace("0001ff", &"00".repeat(MAX_VALUE_BYTES + 1)),
        opaque_document(&"51".repeat(MAX_SUBMITTED_BYTES + 1), part, "")] {
        assert!(source::decode(bad.as_bytes()).is_err());
    }
    // The inclusive native byte ceiling remains usable, not just its refusals.
    let full = opaque_document(&"51".repeat(MAX_SUBMITTED_BYTES),
        &format!(r#"{{"start":0,"end":{MAX_SUBMITTED_BYTES},"kind":"question"}}"#), "");
    assert_eq!(source::decode(full.as_bytes()).unwrap().inputs.opaque().unwrap().submitted_bytes().len(), MAX_SUBMITTED_BYTES);
}
