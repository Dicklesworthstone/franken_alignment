//! Original native recovery cuts and custody; these tests need no pathname
//! listener and do not substitute for the separately authored live service case.
use super::*;
use fa_reference::action::consequence::delivery::persistent::{
    FileDeliverySnapshot, JournalError,
    governance::PolicyUpdate,
    observed::{
        governance::campaigns::{
            FilePolicyCampaignReview, FilePolicyGovernor, FilePolicyPromotionPermit,
        },
        guarded::{
            FileCampaignRequirement, FileGuardSet,
            anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
        },
    },
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::evidence_source::{
    EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource,
};
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::action::{ActionSpec, VERSION};
use fa_reference::{Error, Snapshot};
use std::{
    collections::BTreeMap,
    io::Cursor,
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(crate::test_directory::create(format!(
            "fa-policy-recovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("policy recovery fixture cleanup: {error}");
        }
    }
}

struct Fixture {
    config: Config,
    before: FileDeliverySnapshot,
    document: String,
    governor: FilePolicyGovernor,
    review: FilePolicyCampaignReview,
    key: FilePolicyPromotionPermit,
    root: Directory,
}

fn campaigns() -> FileCampaignRequirement {
    FileCampaignRequirement {
        limits: ReplayLimits {
            cases: 16,
            input_bytes: 1_048_576,
        },
        max_campaigns: 8,
    }
}

fn seed(config: &mut Config, root: &Directory, owner: &mut FileOversight, attempt: u64, tick: u64) {
    let snapshot = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation: 1,
            scope: config.profile.delivery.scope,
        },
        Snapshot {
            semantic_epoch: 1,
            complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]),
        },
        BTreeMap::from([
            ("alpha".into(), b"original alpha view".to_vec()),
            ("beta".into(), b"original beta view".to_vec()),
        ]),
    )
    .unwrap();
    fs::write(root.0.join("source.json"), snapshot.encode()).unwrap();
    owner
        .observe_time(owner.revision(), ElapsedTick(tick))
        .unwrap();
    owner
        .refresh_file_source(owner.revision(), &mut config.source, ElapsedTick(tick))
        .unwrap();
    owner
        .propose(
            owner.revision(),
            attempt,
            ActionSpec {
                version: VERSION,
                scope: config.profile.delivery.scope,
                target: Some(owner.inspect().target),
                payload: b"seed".to_vec(),
                required_witnesses: Vec::new(),
                policy_epoch: owner.inspect().control.ledger.epoch,
                deadline: ElapsedTick(10_000),
                units: 8,
            },
            snapshot.snapshot().clone(),
        )
        .unwrap();
}

impl Fixture {
    fn new() -> Self {
        let root = Directory::new();
        let mut config = Config::decode(include_bytes!(
            "../../../../../fixtures/supervised_publication.json"
        ))
        .unwrap();
        config.store = root.0.join("store");
        config.source = FileEvidenceSource::new(
            root.0.join("source.json"),
            51,
            config.profile.delivery.scope,
            1_048_576,
        )
        .unwrap();
        let guards = FileGuardSet {
            stream: None,
            decoder: None,
            decoder_stop: None,
            source: Some(config.source_policy),
            identity: None,
            campaigns: Some(campaigns()),
            credential: None,
        };
        let (mut owner, roles) = FileOversight::create_guarded_reserved(
            &config.store,
            config.profile.clone(),
            &guards,
            None,
            RecoveryReserve::terminal(),
        )
        .unwrap();
        let governor = roles.policy_governor.unwrap();
        seed(&mut config, &root, &mut owner, 1, 1000);
        let cut = owner.inspect();
        let update = PolicyUpdate::new(
            201,
            cut.control.sequence,
            cut.control.ledger.epoch,
            Policy::new(2, vec![Predicate::PayloadAtMost(64)]).unwrap(),
        )
        .unwrap();
        let review = owner
            .request_policy_campaign(owner.revision(), &update)
            .unwrap();
        assert_eq!(review.report().cases().len(), 1);
        let revision = owner.revision();
        let key = governor
            .approve(&mut owner, revision, &review, false)
            .unwrap();
        owner
            .promote_policy_campaign(owner.revision(), &key)
            .unwrap();
        let before = owner.inspect();
        let anchor = owner.history_anchor().unwrap();
        let mut bytes = Vec::new();
        anchor
            .write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES)
            .unwrap();
        fs::write(root.0.join("before.anchor"), bytes).unwrap();
        let document = format!(
            concat!(
                "{{\"schema_version\":\"fa.guarded-submit/2\",\"minimum\":{{",
                "\"journal_revision\":{},\"control_sequence\":{},\"authority_epoch\":{}}},",
                "\"history_anchor\":{:?},\"retain_history_anchor\":{:?},",
                "\"effective_policy\":{{\"generation\":2,\"nodes\":[{{\"op\":\"payload_at_most\",\"maximum\":64}}]}},",
                "\"campaigns\":{{\"cases\":16,\"input_bytes\":1048576,\"max_campaigns\":8}}}}"
            ),
            before.revision,
            before.control.sequence,
            before.control.ledger.epoch,
            root.0.join("before.anchor").to_str().unwrap(),
            root.0.join("after.anchor").to_str().unwrap()
        );
        drop(owner);
        Self {
            config,
            before,
            document,
            governor,
            review,
            key,
            root,
        }
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

    fn read_anchor(&self) -> FileHistoryAnchor {
        FileHistoryAnchor::read_trusted(
            &mut Cursor::new(fs::read(self.root.0.join("after.anchor")).unwrap()),
            MAX_HISTORY_ANCHOR_BYTES,
        )
        .unwrap()
    }
}

fn plain() -> FilePublicationRecoveryRequirements {
    FilePublicationRecoveryRequirements {
        validation: None,
        feed: None,
        joint: None,
    }
}

#[test]
fn original_promoted_policy_recovers_fresh_custody_and_old_approvals_cannot_return() {
    let mut f = Fixture::new();
    let profile = SubmitProfile::decode(f.document.as_bytes()).unwrap();
    assert_eq!(profile.campaign_requirement(), Some(campaigns()));
    let custody = profile.admit(&f.config).unwrap();
    let before = f.files();
    let mut checkpoints = 0;
    let recovery = ready(&f.config, &profile, &custody, plain(), &mut || {
        checkpoints += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(checkpoints, recovery.progress().total_events + 2);
    assert_eq!(
        f.files(),
        before,
        "Ready must not publish a Fence or clean staging"
    );
    let ReconciledGuardedOwner {
        mut owner,
        roles,
        outcomes,
    } = recovery.finish_reconciled(|| ElapsedTick(1001)).unwrap();
    require_settled(&owner, &outcomes).unwrap();
    assert_eq!(owner.revision(), f.before.revision + 3);
    assert_eq!(owner.current_policy().unwrap().generation(), 2);
    assert_eq!(
        owner.inspect().control.ledger.epoch,
        f.before.control.ledger.epoch + 1
    );
    assert_eq!(
        owner.inspect().control.ledger.charged,
        f.before.control.ledger.charged
    );
    assert_eq!(
        owner.journal_capacity().unwrap().reserve(),
        Some(RecoveryReserve::terminal())
    );
    assert_eq!(
        owner
            .promote_policy_campaign(owner.revision(), &f.key)
            .err(),
        Some(JournalError::Contract(Error::Binding))
    );
    let revision = owner.revision();
    assert_eq!(
        f.governor
            .approve(&mut owner, revision, &f.review, false)
            .err(),
        Some(JournalError::Contract(Error::Binding))
    );
    let fresh = roles.policy_governor.unwrap();
    seed(&mut f.config, &f.root, &mut owner, 2, 1002);
    let cut = owner.inspect();
    let update = PolicyUpdate::new(
        202,
        cut.control.sequence,
        cut.control.ledger.epoch,
        Policy::new(3, vec![Predicate::PayloadAtMost(32)]).unwrap(),
    )
    .unwrap();
    let review = owner
        .request_policy_campaign(owner.revision(), &update)
        .unwrap();
    assert!(review.report().cases().len() >= 2);
    let revision = owner.revision();
    let key = fresh.approve(&mut owner, revision, &review, false).unwrap();
    owner
        .promote_policy_campaign(owner.revision(), &key)
        .unwrap();
    assert_eq!(owner.current_policy().unwrap().generation(), 3);
    guarded::retain(&custody, &owner).unwrap();
    assert_eq!(f.read_anchor(), owner.history_anchor().unwrap());
}

#[test]
fn changed_policy_campaign_inventory_anchor_and_publication_gate_refuse_before_acknowledgment() {
    let f = Fixture::new();
    let before = f.files();
    let foreign = Fixture::new();
    let wrong_anchor = f.document.replace(
        f.root.0.join("before.anchor").to_str().unwrap(),
        foreign.root.0.join("before.anchor").to_str().unwrap(),
    );
    for document in [
        f.document.replace("\"maximum\":64", "\"maximum\":63"),
        f.document
            .replace("\"max_campaigns\":8", "\"max_campaigns\":7"),
        f.document.replace("\"cases\":16", "\"cases\":15"),
        wrong_anchor,
    ] {
        let profile = SubmitProfile::decode(document.as_bytes()).unwrap();
        let custody = profile.admit(&f.config).unwrap();
        assert!(ready(&f.config, &profile, &custody, plain(), &mut || Ok(())).is_err());
        assert_eq!(f.files(), before);
        assert!(!f.root.0.join("after.anchor").exists());
    }
    let profile = SubmitProfile::decode(f.document.as_bytes()).unwrap();
    let custody = profile.admit(&f.config).unwrap();
    let checked = FilePublicationRecoveryRequirements {
        validation: Some(
            fa_reference::action::consequence::delivery::publication_gate::PublicationLimits {
                bindings: 4,
                validation: fa_reference::witness::refinement::RefinementBudget {
                    steps: 32,
                    value_bytes: 128,
                },
            },
        ),
        feed: None,
        joint: None,
    };
    assert!(ready(&f.config, &profile, &custody, checked, &mut || Ok(())).is_err());
    assert_eq!(f.files(), before);
    let accepted = ready(&f.config, &profile, &custody, plain(), &mut || Ok(())).unwrap();
    assert_eq!(accepted.progress().status, FileGuardedRecoveryStatus::Ready);
    drop(accepted);
    assert_eq!(f.files(), before);
}

#[test]
fn every_cooperative_replay_cut_can_stop_without_fence_roles_or_staging_cleanup() {
    let f = Fixture::new();
    let profile = SubmitProfile::decode(f.document.as_bytes()).unwrap();
    let custody = profile.admit(&f.config).unwrap();
    fs::write(
        f.config.store.join("delivery.pending"),
        b"unacknowledged retained bytes",
    )
    .unwrap();
    let before = f.files();
    let total = {
        let run = FileOversight::begin_open_guarded_anchored(
            &f.config.store,
            f.config.profile.clone(),
            &profile.requirements(&f.config),
            custody.anchor(),
        )
        .unwrap();
        run.progress().total_events
    };
    for stop_at in 0..=total + 1 {
        let mut calls = 0;
        let failure = ready(&f.config, &profile, &custody, plain(), &mut || {
            let current = calls;
            calls += 1;
            if current == stop_at {
                Err("original replay deadline".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(failure, "original replay deadline");
        assert_eq!(calls, stop_at + 1);
        assert_eq!(f.files(), before);
        assert!(!f.root.0.join("after.anchor").exists());
    }
    let run = ready(&f.config, &profile, &custody, plain(), &mut || Ok(())).unwrap();
    drop(run);
    assert_eq!(f.files(), before);
}
