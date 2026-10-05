//! Computed probes use the SAME durable policy provider, not a callback bypass.
use super::*;
use crate::action::consequence::delivery::persistent::observed::{driver::learned::FileDriverLearnedLaunch,
    helpers::learned::FileLearnedProbeStatus};
use crate::action::consequence::oversight::{helper_workers::HelperLimits,
    learned_host::sidecar::workers::{LearnedWorkerSchedule, probes::{ProbeReviewLimits, ProbeReviewMember}},
    sidecar::probe_helper::ProbeHelperBudget};

#[test]
fn computed_probe_review_and_publication_share_the_original_durable_policy_gate() {
    let root = Directory::new(); let config = configured(); let (host, reviewer) = owner(&root, &config);
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let mut source = reader(&root); publish(&root.store().with_extension("policy"), &capture(1));
    supervisor.prepare_learned_policy_intake(&mut source, || ElapsedTick(1)).result.unwrap();
    let ticket = port.submit(71, intent()).unwrap(); let mut driver = FileSupervisedDriver::new(supervisor);
    let sidecar = sidecar(&mut driver, 71);
    let members = {
        let host = driver.supervisor().host().unwrap(); let original = host.checked_learned_sidecar(&sidecar).unwrap();
        let rows: BTreeSet<_> = original.source().groups().map(|group| group.row).collect();
        ["alpha", "beta"].into_iter().enumerate().map(|(index, member)| {
            let probes = rows.iter().map(|row| {
                let (frame, heads, channels) = original.source().row_shape(*row).unwrap();
                (*row, vec![LinearProbe::new(1, 1, frame.profile, &vec![0.0; heads * channels], 0.0, 1.0).unwrap()])
            }).collect();
            (member.to_owned(), ProbeReviewMember { probes, salts: vec![vec![16 + index as u8; 32]] })
        }).collect()
    };
    driver.start_learned_probe_review(FileDriverLearnedLaunch { request: 71, sidecar,
        schedule: LearnedWorkerSchedule { rounds: vec![LearnedWorkerRound { round: 101, evidence_root: [7; 32],
            window: ReviewWindow { commit_by: ElapsedTick(10), reveal_by: ElapsedTick(15) } }],
            helpers: HelperLimits::default(), polls: 64 }, members,
        limits: ProbeReviewLimits { evaluations: 2, per_evaluation: ProbeHelperBudget::default() } },
        snapshot(), ElapsedTick(1)).unwrap();
    for _ in 0..64 {
        if driver.phase() != (FileDriverPhase::Reviewing { request: 71 }) { break; }
        let report = driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), None);
        assert_eq!(report.source_updates, vec![Ok(capture(1).identity())]);
        report.result.unwrap();
    }
    assert_eq!(driver.learned_probe_review().unwrap().review().status(), FileLearnedProbeStatus::Finished);
    assert_eq!(driver.phase(), FileDriverPhase::AwaitingDispatch { request: 71 });
    assert!(driver.learned_probe_review().unwrap().review().records().values()
        .any(|record| record.work.evaluated_probes > 0));
    let report = driver.request_learned_human_approval_from_policy_file(&mut source, 1001,
        ElapsedTick(80), || ElapsedTick(1));
    assert_eq!(report.source_updates, vec![Ok(capture(1).identity())]); let request = report.result.unwrap();
    let human = {
        let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        reviewer.approve(&mut host, revision, &request).unwrap()
    };
    let report = driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), Some(&human));
    assert_eq!(report.source_updates, vec![Ok(capture(1).identity()); 2]);
    assert!(matches!(report.result.unwrap(), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), None).result.unwrap(),
        FileDriverEvent::PublicationChecked { .. }));
    let reads = source.status().read_attempts;
    assert!(matches!(driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), None).result.unwrap(),
        FileDriverEvent::Reconciled { .. }));
    assert_eq!(source.status().read_attempts, reads);
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let stored = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    assert_eq!(stored.payload, b"aa"); assert_eq!(stored.executions, 1);
}
