//! Exact independent expectations exercise the original native recovery fence.
//! This is not a substitute for live governor/actor service composition tests.
use super::*;
use fa_reference::action::consequence::delivery::persistent::{
    JournalError, RecoveryReserve,
    governance::PolicyUpdate,
    observed::{
        FileOversight,
        guarded::{
            FileOversightRoles,
            anchored::MAX_HISTORY_ANCHOR_BYTES,
            recovery::{FileGuardedRecovery, FileGuardedRecoveryStatus},
        },
    },
};
use fa_reference::action::consequence::gate::containment::session::policy::Predicate;
use fa_reference::action::consequence::oversight::{
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_governance::CampaignDisposition,
};
use fa_reference::action::{ActionSpec, ElapsedTick, VERSION};
use fa_reference::{Error, Snapshot};
use std::{
    collections::BTreeMap,
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

const TEMPLATE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/fixtures/supervised_publication.json"
));

struct Fixture {
    root: PathBuf,
    config: Config,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = crate::test_directory::create(format!(
            "fa-submit-policy-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut config = Config::decode(TEMPLATE).unwrap();
        config.store = root.join("store");
        Self { root, config }
    }

    fn guards(&self) -> FileGuardSet {
        FileGuardSet {
            stream: None,
            decoder: None,
            decoder_stop: None,
            source: Some(self.config.source_policy),
            identity: None,
            campaigns: Some(FileCampaignRequirement {
                limits: ReplayLimits {
                    cases: 16,
                    input_bytes: 1_048_576,
                },
                max_campaigns: 8,
            }),
            credential: None,
        }
    }

    fn create(&self) -> (FileOversight, FileOversightRoles) {
        FileOversight::create_guarded_reserved(
            &self.config.store,
            self.config.profile.clone(),
            &self.guards(),
            None,
            RecoveryReserve::terminal(),
        )
        .unwrap()
    }

    fn seed(&self, host: &mut FileOversight, attempt: u64, tick: u64) {
        let identity = EvidenceIdentity {
            source: self.config.source_policy.source.source,
            generation: self.config.source_policy.source.generation,
            scope: self.config.profile.delivery.scope,
        };
        let capture = EvidenceSnapshot::new(
            identity,
            Snapshot {
                semantic_epoch: 1,
                complete: true,
                values: BTreeMap::from([(7, b"ok".to_vec())]),
            },
            BTreeMap::from([
                ("alpha".into(), b"retained case".to_vec()),
                ("beta".into(), b"retained case".to_vec()),
            ]),
        )
        .unwrap();
        let path = self.root.join("source.json");
        fs::write(&path, capture.encode()).unwrap();
        let mut source =
            FileEvidenceSource::new(path, identity.source, identity.scope, 1_048_576).unwrap();
        host.observe_time(host.revision(), ElapsedTick(tick))
            .unwrap();
        host.refresh_file_source(host.revision(), &mut source, ElapsedTick(tick))
            .unwrap();
        host.propose(
            host.revision(),
            attempt,
            ActionSpec {
                version: VERSION,
                scope: self.config.profile.delivery.scope,
                target: Some(host.inspect().target),
                payload: b"case".to_vec(),
                required_witnesses: Vec::new(),
                policy_epoch: host.inspect().control.ledger.epoch,
                deadline: ElapsedTick(10_000),
                units: 8,
            },
            capture.snapshot().clone(),
        )
        .unwrap();
    }

    fn retain(&self, host: &FileOversight) -> String {
        let mut bytes = Vec::new();
        host.history_anchor()
            .unwrap()
            .write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES)
            .unwrap();
        fs::write(self.root.join("before.anchor"), bytes).unwrap();
        let cut = host.inspect();
        document(
            FileRecoveryFloor {
                journal_revision: cut.revision,
                control_sequence: cut.control.sequence,
                authority_epoch: cut.control.ledger.epoch,
            },
            &self.root.join("before.anchor"),
            &self.root.join("after.anchor"),
        )
    }

    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(&self.config.store)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file())
            .map(|path| {
                let bytes = fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!("submit policy fixture cleanup: {error}");
        }
    }
}

fn document(minimum: FileRecoveryFloor, input: &Path, output: &Path) -> String {
    format!(
        r#"{{"schema_version":"fa.guarded-submit/2","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},"history_anchor":{:?},"retain_history_anchor":{:?},"effective_policy":{{"generation":2,"nodes":[{{"op":"payload_at_most","maximum":64}}]}},"campaigns":{{"cases":16,"input_bytes":1048576,"max_campaigns":8}}}}"#,
        minimum.journal_revision,
        minimum.control_sequence,
        minimum.authority_epoch,
        input.to_str().unwrap(),
        output.to_str().unwrap()
    )
}

fn legacy_document(minimum: FileRecoveryFloor, input: &Path) -> String {
    format!(
        r#"{{"schema_version":"fa.guarded-submit/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},"history_anchor":{:?}}}"#,
        minimum.journal_revision,
        minimum.control_sequence,
        minimum.authority_epoch,
        input.to_str().unwrap()
    )
}

fn update(host: &FileOversight, operation: u64, generation: u64, maximum: usize) -> PolicyUpdate {
    let cut = host.inspect();
    PolicyUpdate::new(
        operation,
        cut.control.sequence,
        cut.control.ledger.epoch,
        Policy::new(generation, vec![Predicate::PayloadAtMost(maximum)]).unwrap(),
    )
    .unwrap()
}

fn advance(run: &mut FileGuardedRecovery) -> Result<(), JournalError> {
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        let progress = run.progress();
        let next = run.advance(progress.replayed_events, 1)?;
        assert_eq!(next.replayed_events, progress.replayed_events + 1);
    }
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Ready);
    Ok(())
}

#[test]
fn exact_promoted_policy_recovers_a_fresh_governor_and_revokes_old_campaign_approval() {
    let fixture = Fixture::new();
    let (mut host, roles) = fixture.create();
    let old_governor = roles.policy_governor.unwrap();
    fixture.seed(&mut host, 1, 1);
    let candidate = update(&host, 91, 2, 64);
    let review = host
        .request_policy_campaign(host.revision(), &candidate)
        .unwrap();
    assert_eq!(review.report().cases().len(), 1);
    let revision = host.revision();
    let permit = old_governor
        .approve(&mut host, revision, &review, false)
        .unwrap();
    host.promote_policy_campaign(host.revision(), &permit)
        .unwrap();
    fixture.seed(&mut host, 2, 2);
    let pending = update(&host, 92, 3, 32);
    let old_review = host
        .request_policy_campaign(host.revision(), &pending)
        .unwrap();
    let revision = host.revision();
    let old_permit = old_governor
        .approve(&mut host, revision, &old_review, false)
        .unwrap();
    assert_eq!(
        host.policy_campaign(92).unwrap().observed_disposition(),
        CampaignDisposition::Approved
    );
    let text = fixture.retain(&host);
    let before = host.inspect();
    let original_anchor = fs::read(fixture.root.join("before.anchor")).unwrap();
    drop(host);

    let profile = SubmitProfile::decode(text.as_bytes()).unwrap();
    assert_eq!(profile.campaign_requirement(), fixture.guards().campaigns);
    let custody = profile.admit(&fixture.config).unwrap();
    let mut run = FileOversight::begin_open_guarded_anchored(
        &fixture.config.store,
        fixture.config.profile.clone(),
        &profile.requirements(&fixture.config),
        custody.anchor(),
    )
    .unwrap()
    .require_recovery_reserve(RecoveryReserve::terminal())
    .unwrap();
    advance(&mut run).unwrap();
    let recovered = run.finish_reconciled(|| ElapsedTick(3)).unwrap();
    let mut host = recovered.owner;
    let fresh_governor = recovered.roles.policy_governor.unwrap();
    assert_eq!(host.revision(), before.revision + 3);
    assert_eq!(
        host.current_policy().unwrap(),
        &Policy::new(2, vec![Predicate::PayloadAtMost(64)]).unwrap()
    );
    assert_eq!(
        host.inspect().control.ledger.epoch,
        before.control.ledger.epoch + 1
    );
    assert_eq!(host.inspect().executions, before.executions);
    assert_eq!(
        host.inspect().control.ledger.charged,
        before.control.ledger.charged
    );
    assert_eq!(
        host.policy_campaign(92).unwrap().observed_disposition(),
        CampaignDisposition::Revoked
    );
    let revision = host.revision();
    assert_eq!(
        old_governor
            .approve(&mut host, revision, &old_review, false)
            .err(),
        Some(JournalError::Contract(Error::Binding))
    );
    assert_eq!(
        host.promote_policy_campaign(host.revision(), &old_permit),
        Err(JournalError::Contract(Error::Binding))
    );
    assert_eq!(host.revision(), revision);

    // A separately provisioned fresh role still needs a fresh, original campaign
    // and current source. Neither importing the profile nor replay approves it.
    fixture.seed(&mut host, 3, 4);
    let candidate = update(&host, 93, 3, 32);
    let review = host
        .request_policy_campaign(host.revision(), &candidate)
        .unwrap();
    let revision = host.revision();
    let permit = fresh_governor
        .approve(&mut host, revision, &review, false)
        .unwrap();
    host.promote_policy_campaign(host.revision(), &permit)
        .unwrap();
    assert_eq!(host.current_policy().unwrap().generation(), 3);
    assert_eq!(
        host.journal_capacity().unwrap().reserve(),
        Some(RecoveryReserve::terminal())
    );
    assert_eq!(host.inspect().executions, before.executions);
    custody
        .retain(&host.history_anchor_after(custody.anchor()).unwrap())
        .unwrap();
    assert_eq!(
        fs::read(fixture.root.join("before.anchor")).unwrap(),
        original_anchor
    );
    assert!(fixture.root.join("after.anchor").is_file());
}

#[test]
fn policy_campaign_source_floor_reserve_and_anchor_mismatches_leave_original_files_untouched() {
    let fixture = Fixture::new();
    let (mut host, roles) = fixture.create();
    fixture.seed(&mut host, 1, 1);
    let candidate = update(&host, 91, 2, 64);
    let review = host
        .request_policy_campaign(host.revision(), &candidate)
        .unwrap();
    let revision = host.revision();
    let permit = roles
        .policy_governor
        .unwrap()
        .approve(&mut host, revision, &review, false)
        .unwrap();
    host.promote_policy_campaign(host.revision(), &permit)
        .unwrap();
    let text = fixture.retain(&host);
    drop(host);
    fs::write(
        fixture.config.store.join("delivery.pending"),
        b"preserve refused recovery",
    )
    .unwrap();
    let unchanged = fixture.files();
    let profile = SubmitProfile::decode(text.as_bytes()).unwrap();
    let custody = profile.admit(&fixture.config).unwrap();
    let expected = profile.requirements(&fixture.config);
    for mismatch in 0..9 {
        let mut wrong = expected.clone();
        match mismatch {
            0 => {
                wrong.effective_policy = Policy::new(2, vec![Predicate::PayloadAtMost(32)]).unwrap()
            }
            1 => wrong.guards.campaigns = None,
            2 => wrong.guards.campaigns.as_mut().unwrap().limits.cases += 1,
            3 => wrong.guards.campaigns.as_mut().unwrap().limits.input_bytes += 1,
            4 => wrong.guards.campaigns.as_mut().unwrap().max_campaigns += 1,
            5 => wrong.guards.source.as_mut().unwrap().source.generation += 1,
            6 => wrong.minimum.journal_revision += 1,
            7 => wrong.minimum.control_sequence += 1,
            _ => wrong.minimum.authority_epoch += 1,
        }
        let refused = FileOversight::begin_open_guarded_anchored(
            &fixture.config.store,
            fixture.config.profile.clone(),
            &wrong,
            custody.anchor(),
        )
        .and_then(|mut run| {
            advance(&mut run)?;
            run.finish()
        });
        assert!(refused.is_err(), "independent requirement {mismatch}");
        assert_eq!(fixture.files(), unchanged);
        assert!(!fixture.root.join("after.anchor").exists());
    }
    let mut wrong = RecoveryReserve::terminal();
    wrong.bytes += 1;
    assert!(
        FileOversight::begin_open_guarded_anchored(
            &fixture.config.store,
            fixture.config.profile.clone(),
            &expected,
            custody.anchor()
        )
        .unwrap()
        .require_recovery_reserve(wrong)
        .is_err()
    );
    assert_eq!(fixture.files(), unchanged);
    let foreign = Fixture::new();
    let (foreign_host, _) = foreign.create();
    let foreign_anchor = foreign_host.history_anchor().unwrap();
    drop(foreign_host);
    assert!(
        FileOversight::begin_open_guarded_anchored(
            &fixture.config.store,
            fixture.config.profile.clone(),
            &expected,
            &foreign_anchor
        )
        .is_err()
    );
    assert_eq!(fixture.files(), unchanged);
    let legacy = SubmitProfile::decode(
        legacy_document(profile.minimum(), &fixture.root.join("before.anchor")).as_bytes(),
    )
    .unwrap();
    let mut run = FileOversight::begin_open_guarded_anchored(
        &fixture.config.store,
        fixture.config.profile.clone(),
        &legacy.requirements(&fixture.config),
        custody.anchor(),
    )
    .unwrap();
    assert!(advance(&mut run).is_err());
    drop(run);
    assert_eq!(fixture.files(), unchanged);
    // The paired exact expectation remains permitted after every refusal.
    let mut run = FileOversight::begin_open_guarded_anchored(
        &fixture.config.store,
        fixture.config.profile.clone(),
        &expected,
        custody.anchor(),
    )
    .unwrap()
    .require_recovery_reserve(RecoveryReserve::terminal())
    .unwrap();
    advance(&mut run).unwrap();
    let (host, roles) = run.finish().unwrap();
    assert!(roles.policy_governor.is_some());
    assert_eq!(host.current_policy().unwrap(), &expected.effective_policy);
    assert!(!fixture.config.store.join("delivery.pending").exists());
}

#[test]
fn closed_v2_and_unchanged_legacy_parser_do_not_infer_missing_expectations() {
    let fixture = Fixture::new();
    let floor = FileRecoveryFloor {
        journal_revision: 4,
        control_sequence: 3,
        authority_epoch: 2,
    };
    let text = document(
        floor,
        &fixture.root.join("before.anchor"),
        &fixture.root.join("after.anchor"),
    );
    let profile = SubmitProfile::decode(text.as_bytes()).unwrap();
    assert_eq!(profile.requirements(&fixture.config).minimum, floor);
    assert_eq!(
        profile.requirements(&fixture.config).guards,
        fixture.guards()
    );
    assert!(profile.require_legacy().is_err());
    for invalid in [
        text.replace("fa.guarded-submit/2", "fa.guarded-submit/3"),
        text.replace("fa.guarded-submit/2", "fa.guarded-stop/2"),
        text.replace("\"effective_policy\":", "\"unexpected_policy\":"),
        text.replace("\"campaigns\":", "\"unexpected_campaigns\":"),
        text.replace("\"cases\":16", "\"cases\":0"),
        text.replace("\"cases\":16", "\"cases\":5121"),
        text.replace("\"cases\":16", "\"cases\":16,\"cases\":16"),
        text.replace("\"cases\":16", "\"cases\":\"16\""),
        text.replace("\"input_bytes\":1048576", "\"input_bytes\":33554433"),
        text.replace("\"input_bytes\":1048576", "\"input_bytes\":0"),
        text.replace("\"max_campaigns\":8", "\"max_campaigns\":65"),
        text.replace("\"max_campaigns\":8", "\"max_campaigns\":0"),
        text.replace(
            "\"max_campaigns\":8",
            "\"max_campaigns\":8,\"approve\":true",
        ),
        text.replace("\"maximum\":64", "\"maximum\":64,\"ignored\":true"),
        text.replace("\"generation\":2", "\"generation\":0"),
        text.replace(
            "\"campaigns\":{\"cases\":16,\"input_bytes\":1048576,\"max_campaigns\":8}",
            "\"campaigns\":null",
        ),
        text.replacen('{', "{\"authority\":true,", 1),
        text.replace(
            fixture.root.join("before.anchor").to_str().unwrap(),
            "relative.anchor",
        ),
        text.replace(
            fixture.root.join("before.anchor").to_str().unwrap(),
            &format!("/{}", "x".repeat(4096)),
        ),
    ] {
        assert!(
            SubmitProfile::decode(invalid.as_bytes()).is_err(),
            "accepted malformed profile"
        );
    }
    assert!(super::super::Profile::decode(text.as_bytes()).is_err());
    assert!(super::super::ResumeProfile::decode(text.as_bytes()).is_err());
    let legacy = legacy_document(floor, &fixture.root.join("before.anchor"));
    let old = SubmitProfile::decode(legacy.as_bytes()).unwrap();
    old.require_legacy().unwrap();
    assert_eq!(old.campaign_requirement(), None);
    assert_eq!(
        old.requirements(&fixture.config).effective_policy,
        fixture.config.profile.delivery.policy
    );
    assert_eq!(old.requirements(&fixture.config).minimum, floor);
    let too_long = legacy.replace(
        fixture.root.join("before.anchor").to_str().unwrap(),
        &format!("/{}", "x".repeat(4096)),
    );
    assert!(SubmitProfile::decode(too_long.as_bytes()).is_err());
    let oversized = format!("{legacy}{}", " ".repeat(16 * 1024));
    assert!(SubmitProfile::decode(oversized.as_bytes()).is_err());
    let unknown = legacy.replacen('{', "{\"effective_policy\":null,", 1);
    assert!(SubmitProfile::decode(unknown.as_bytes()).is_err());
    assert!(!fixture.config.store.exists());
}
