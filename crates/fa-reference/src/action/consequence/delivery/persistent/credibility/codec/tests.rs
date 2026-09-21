use super::*;

fn manifest(cases: usize, helpers: usize) -> Campaign {
    Campaign { scope: EvaluationScope { campaign: 1, model_generation: 1, evaluator_generation: 1,
        held_out_manifest: [9; 32] }, label_owner: "external-truth".into(),
        helpers: (0..helpers).map(|id| (format!("h{id:03}"), HelperGeneration {
            generation: 1, cohort: format!("c{id:03}") })).collect(),
        strata: BTreeSet::from(["publication".into()]),
        cases: (1..=cases).map(|id| {
            let mut root = [0; 32]; root[..8].copy_from_slice(&(id as u64).to_be_bytes());
            CaseSpec { id: id as u64, stratum: "publication".into(), evidence_root: root, dispatch_sequence: 2 }
        }).collect() }
}
fn wire(snapshot: &CredibilitySnapshot) -> Vec<u8> {
    let mut w = Writer::new(MAX_ACTIVATION_BYTES);
    write_snapshot(&mut w, snapshot).unwrap(); w.finish()
}
fn restore(bytes: &[u8]) -> Result<CredibilitySnapshot, Error> {
    let mut r = Reader::new(bytes);
    let snapshot = read_snapshot(&mut r)?;
    r.end()?;
    Ok(snapshot)
}

#[test]
fn absent_reports_explicit_missing_abstention_and_censoring_survive_distinctly() {
    let campaign = manifest(4, 2);
    let mut ledger = CredibilityLedger::new(campaign.clone()).unwrap();
    for (id, observation) in [(2, Observation::Missing), (3, Observation::Abstain), (4, Observation::Clear)] {
        ledger.record_observations(id, campaign.helpers.keys().map(|name| (name.clone(), observation)).collect()).unwrap();
    }
    ledger.record_label(3, EvaluationLabel { owner: campaign.label_owner.clone(), evaluator_generation: 1,
        source: LabelSource::IndependentEvaluation, evidence_root: campaign.cases[2].evidence_root,
        recorded_sequence: 3, verdict: LabelVerdict::Censored }).unwrap();
    ledger.record_label(4, EvaluationLabel { owner: campaign.label_owner, evaluator_generation: 1,
        source: LabelSource::IndependentEvaluation, evidence_root: campaign.cases[3].evidence_root,
        recorded_sequence: 3, verdict: LabelVerdict::Safe }).unwrap();
    let snapshot = ledger.seal(3).unwrap();
    let decoded = restore(&wire(&snapshot)).unwrap();
    assert_eq!(decoded, snapshot);
    assert!(decoded.case_observations(1).is_none());
    assert_eq!(decoded.case_observations(2).unwrap()["h000"], Observation::Missing);
    let score = &decoded.scores()["h000"]["publication"];
    assert_eq!((score.cases, score.pending, score.censored, score.missing, score.abstained, score.safe),
        (4, 2, 1, 2, 1, 1));
}

#[test]
fn label_owner_source_root_and_order_are_revalidated_not_deserialized_as_scores() {
    let campaign = manifest(1, 1);
    let mut ledger = CredibilityLedger::new(campaign.clone()).unwrap();
    ledger.record_observations(1, BTreeMap::from([("h000".into(), Observation::Hold { first_sequence: 1 })])).unwrap();
    ledger.record_label(1, EvaluationLabel { owner: campaign.label_owner, evaluator_generation: 1,
        source: LabelSource::IndependentEvaluation, evidence_root: campaign.cases[0].evidence_root,
        recorded_sequence: 2, verdict: LabelVerdict::Violation }).unwrap();
    let snapshot = ledger.seal(2).unwrap();
    let bytes = wire(&snapshot);
    assert_eq!(restore(&bytes).unwrap(), snapshot);
    let owner = b"external-truth";
    let positions: Vec<_> = bytes.windows(owner.len()).enumerate()
        .filter_map(|(i, bytes)| (bytes == owner).then_some(i)).collect();
    assert_eq!(positions.len(), 2);
    let label = positions[1];
    for lane in 0..4 {
        let mut changed = bytes.clone();
        match lane {
            0 => changed[label] = b'X',
            1 => changed[label + owner.len() + 8] = 1, // CommitteeConsensus is not independent truth.
            2 => changed[label + owner.len() + 9] ^= 1,
            _ => changed[label + owner.len() + 9 + 32..label + owner.len() + 9 + 40]
                .copy_from_slice(&1_u64.to_be_bytes()),
        }
        assert!(restore(&changed).is_err());
    }
}

#[test]
fn product_limit_is_checked_before_decoding_case_storage() {
    let exact = CredibilityLedger::new(manifest(512, 128)).unwrap().seal(2).unwrap();
    assert_eq!(restore(&wire(&exact)).unwrap(), exact);
    let over = CredibilityLedger::new(manifest(513, 128)).unwrap().seal(2).unwrap();
    let mut w = Writer::new(MAX_ACTIVATION_BYTES);
    assert_eq!(write_snapshot(&mut w, &over), Err(Error::Limit));
    // A hostile declared count is refused before a single case can be retained.
    let campaign = manifest(1, 128);
    let mut header = Writer::new(MAX_ACTIVATION_BYTES);
    write_scope(&mut header, &campaign.scope).unwrap();
    text(&mut header, &campaign.label_owner).unwrap();
    write_helpers(&mut header, &campaign.helpers).unwrap();
    write_strata(&mut header, &campaign.strata).unwrap();
    header.count(513).unwrap();
    assert_eq!(read_snapshot(&mut Reader::new(&header.finish())), Err(Error::Limit));
}

#[test]
fn duplicate_roster_names_and_oversized_strings_are_rejected() {
    let mut w = Writer::new(MAX_ACTIVATION_BYTES);
    w.count(2).unwrap();
    for _ in 0..2 { text(&mut w, "helper").unwrap(); w.u64(1).unwrap(); text(&mut w, "cohort").unwrap(); }
    assert_eq!(read_helpers(&mut Reader::new(&w.finish())), Err(Error::InvalidInput));
    let mut w = Writer::new(MAX_ACTIVATION_BYTES);
    text(&mut w, &"a".repeat(MAX_IDENTIFIER_BYTES)).unwrap();
    assert_eq!(read_text(&mut Reader::new(&w.finish())).unwrap().len(), MAX_IDENTIFIER_BYTES);
    let mut w = Writer::new(MAX_ACTIVATION_BYTES);
    assert_eq!(text(&mut w, &"a".repeat(MAX_IDENTIFIER_BYTES + 1)), Err(Error::Limit));
    let mut w = Writer::new(MAX_ACTIVATION_BYTES);
    w.count(MAX_IDENTIFIER_BYTES + 1).unwrap();
    assert_eq!(read_text(&mut Reader::new(&w.finish())), Err(Error::Limit));
}
