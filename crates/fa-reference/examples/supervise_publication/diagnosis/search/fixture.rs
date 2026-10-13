//! Actual source-publication histories with two independent exact-policy blockers.
use super::*;
use crate::config::Config;
use crate::recover_stop::guarded::Profile;
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, VERSION};
use fa_reference::action::consequence::delivery::persistent::{RecoveryReserve, requests::FileRequestDisposition};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{FileRecoveryFloor, anchored::MAX_HISTORY_ANCHOR_BYTES};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use fa_reference::Snapshot;
use std::{collections::BTreeMap, fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Case {
    pub root: PathBuf, pub config: Config, pub host: FileOversight,
    pub config_file: PathBuf, pub profile_file: PathBuf, pub anchor: PathBuf,
}
impl Case {
    pub(super) fn new() -> Self { Self::with_policy(None) }
    pub(super) fn with_policy(policy: Option<fa_reference::action::consequence::gate::containment::session::policy::Policy>) -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-search-command-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        let text = include_str!("../../../../fixtures/supervised_publication.json")
            .replace("/operator", root.to_str().unwrap())
            .replacen("\"maximum\": 512", "\"maximum\": 4", 1)
            .replacen("\"maximum\": 512", "\"maximum\": 1", 1);
        let config_file = root.join("config.json"); fs::write(&config_file, text).unwrap();
        let mut config = Config::read(&config_file).unwrap();
        // Alternative ORIGINAL native policies exercise nonmonotone and unknown
        // outcomes through the loaded-owner seam, not the unchanged CLI file.
        if let Some(policy) = policy { config.profile.delivery.policy = policy; }
        let snapshot = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) };
        let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope: config.profile.delivery.scope },
            snapshot.clone(), ["alpha", "beta"].into_iter().map(|m| (m.to_owned(), b"original source".to_vec())).collect()).unwrap();
        let evidence = root.join("evidence.json"); fs::write(&evidence, data.encode()).unwrap();
        let (mut host, _) = FileOversight::create(&config.store, config.profile.clone()).unwrap();
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
        host.enable_file_source(host.revision(), config.source_policy).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1)).unwrap();
        let spec = |payload: &[u8], units| ActionSpec { version: VERSION, scope: config.profile.delivery.scope,
            target: Some(config.profile.delivery.target), payload: payload.to_vec(), required_witnesses: vec![],
            policy_epoch: 0, deadline: ElapsedTick(100000), units };
        host.propose(host.revision(), 1, spec(b"prior", 2), snapshot.clone()).unwrap();
        let denied = host.submit_request(host.revision(), 901, spec(b"blocked", 2), snapshot.clone()).unwrap();
        assert_eq!(denied.disposition, FileRequestDisposition::Admitted { attempt: 2, stage: ActionState::Denied });
        let allowed = host.submit_request(host.revision(), 2, spec(b"ok", 1), snapshot).unwrap();
        assert_eq!(allowed.disposition, FileRequestDisposition::Admitted { attempt: 3, stage: ActionState::Reviewing });
        host.cancel_request(host.revision(), 2).unwrap();
        host.withdraw_file_source(host.revision()).unwrap(); host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        fs::remove_file(evidence).unwrap(); config.programs.clear();
        let view = host.inspect();
        let minimum = FileRecoveryFloor { journal_revision: view.revision,
            control_sequence: view.control.sequence, authority_epoch: view.control.ledger.epoch };
        let mut bytes = Vec::new(); host.history_anchor().unwrap().write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
        let anchor = root.join("retained.anchor"); fs::write(&anchor, bytes).unwrap();
        let profile_file = root.join("read.json");
        fs::write(&profile_file, format!(concat!("{{\"schema_version\":\"fa.guarded-stop/1\",\"minimum\":{{",
            "\"journal_revision\":{},\"control_sequence\":{},\"authority_epoch\":{}}},\"history_anchor\":{:?}}}"),
            minimum.journal_revision, minimum.control_sequence, minimum.authority_epoch, anchor.to_str().unwrap())).unwrap();
        fs::write(config.store.join("delivery.pending"), b"preserve original staging").unwrap();
        Self { root, config, host, config_file, profile_file, anchor }
    }
    pub(super) fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(&self.config.store).unwrap().map(|v| v.unwrap().path()).filter(|p| p.is_file())
            .chain([self.anchor.clone()]).map(|p| { let b = fs::read(&p).unwrap(); (p,b) }).collect()
    }
    pub(super) fn args(&self, request: u64, menu: &str) -> Vec<String> {
        let file = self.root.join("menu.json"); fs::write(&file, menu).unwrap();
        vec!["explain-proposal".into(), self.config_file.to_str().unwrap().into(), request.to_string(),
            self.profile_file.to_str().unwrap().into(), "--search".into(), file.to_str().unwrap().into()]
    }
    pub(super) fn investigation(&self, request: u64, menu: &Menu) -> FileProposalInvestigation {
        let profile = Profile::read(&self.profile_file).unwrap();
        super::super::super::read_with_elapsed(&self.config, &profile, request, menu.scope.clone(), &mut || Duration::ZERO).unwrap()
    }
}
impl Drop for Case {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.root) { eprintln!("search cleanup: {error}"); } }
}
pub(super) fn document(candidates: &str) -> String {
    let max = maximum();
    format!(concat!("{{\"schema_version\":\"fa.proposal-search/1\",",
        "\"scope\":{{\"id\":81,\"payload\":true,\"target\":true,\"units\":true,\"keys\":[7]}},",
        "\"budget\":{{\"cases\":{},\"retained_edit_bytes\":{}}},",
        "\"candidates\":[{}]}}"), max.cases, max.retained_edit_bytes, candidates)
}
pub(super) const CANDIDATES: &str = r#"{"kind":"payload","bytes_hex":"6f6b"},{"kind":"payload","bytes_hex":"796f"},{"kind":"units","value":1},{"kind":"key","key":7,"expected_hex":"6f6b","replacement_hex":"6e6f"}"#;
