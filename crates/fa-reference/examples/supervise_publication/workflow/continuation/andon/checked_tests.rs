use super::*;
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::FileRecoveryFloor;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = crate::test_directory::create(format!(
            "fa-checked-andon-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn publication(root: &Directory) -> PublicationProfile {
    PublicationProfile::decode(
        format!(
            r#"{{
      "schema":"fa.supervised-witnesses/1",
      "source":91,
      "original":"{}",
      "current":"{}",
      "limits":{{"bindings":8,"steps":32,"value_bytes":4096}},
      "requests":[{{"kind":"exact_value","key":7,"role":"policy_input"}}]
    }}"#,
            root.0.join("original.witness").display(),
            root.0.join("current.witness").display()
        )
        .as_bytes(),
    )
    .unwrap()
}
fn settings() -> Settings {
    Settings::decode(include_bytes!(
        "../../../../../fixtures/native_andon_operator.json"
    ))
    .unwrap()
}
fn config(root: &Directory) -> Config {
    let mut config = Config::decode(include_bytes!(
        "../../../../../fixtures/supervised_publication.json"
    ))
    .unwrap();
    config.store = root.0.join("store");
    config
}

#[test]
fn checked_first_image_retains_publication_and_native_accounting_profiles_together() {
    let root = Directory::new();
    let config = config(&root);
    let settings = settings();
    let publication = publication(&root);
    let limits = publication.simple_limits().unwrap();
    let (host, _roles) = settings
        .create_checked(&config, limits, RecoveryReserve::terminal())
        .unwrap();
    assert_eq!(host.publication_validation_profile().unwrap(), Some(limits));
    assert_eq!(
        host.journal_capacity().unwrap().reserve(),
        Some(RecoveryReserve::terminal())
    );
    assert_eq!(
        host.andon_binding().unwrap().identity(),
        settings.config.identity()
    );
    let anchor = host.history_anchor().unwrap();
    let state = host.inspect();
    drop(host);
    let expected = settings
        .requirements(
            &config,
            FileRecoveryFloor {
                journal_revision: state.revision,
                control_sequence: state.control.sequence,
                authority_epoch: state.control.ledger.epoch,
            },
        )
        .unwrap()
        .require_publication(limits)
        .unwrap();
    let (recovered, _roles) = FileOversight::open_andon_guarded(
        &config.store,
        config.profile.clone(),
        &expected,
        &settings.protocol,
        &anchor,
    )
    .unwrap();
    assert_eq!(
        recovered.publication_validation_profile().unwrap(),
        Some(limits)
    );
    assert_eq!(
        recovered.andon_binding().unwrap().identity(),
        settings.config.identity()
    );
    assert!(!recovered.clock_ready());
}

#[test]
fn checked_recovery_cannot_omit_or_drift_the_publication_profile() {
    let root = Directory::new();
    let config = config(&root);
    let settings = settings();
    let publication = publication(&root);
    let limits = publication.simple_limits().unwrap();
    let (host, _) = settings
        .create_checked(&config, limits, RecoveryReserve::terminal())
        .unwrap();
    let anchor = host.history_anchor().unwrap();
    let state = host.inspect();
    drop(host);
    let base = settings
        .requirements(
            &config,
            FileRecoveryFloor {
                journal_revision: state.revision,
                control_sequence: state.control.sequence,
                authority_epoch: state.control.ledger.epoch,
            },
        )
        .unwrap();
    assert!(
        FileOversight::open_andon_guarded(
            &config.store,
            config.profile.clone(),
            &base,
            &settings.protocol,
            &anchor
        )
        .is_err()
    );
    let changed =
        fa_reference::action::consequence::delivery::publication_gate::PublicationLimits {
            bindings: limits.bindings + 1,
            validation: limits.validation,
        };
    let wrong = base.require_publication(changed).unwrap();
    assert!(
        FileOversight::open_andon_guarded(
            &config.store,
            config.profile.clone(),
            &wrong,
            &settings.protocol,
            &anchor
        )
        .is_err()
    );
}

#[test]
fn stronger_publication_history_profiles_refuse_instead_of_downgrading() {
    let root = Directory::new();
    let profile = PublicationProfile::decode(
        format!(
            r#"{{
      "schema":"fa.supervised-witnesses/4","source":91,
      "original":"{}","current":"{}","max_retries":2,
      "feed":{{"source":41,"path":"{}","after":0,"clock":"unix_milliseconds",
        "max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},
      "limits":{{"bindings":8,"steps":32,"value_bytes":4096}},
      "requests":[{{"kind":"exact_value","key":7,"role":"policy_input"}}]
    }}"#,
            root.0.join("o").display(),
            root.0.join("c").display(),
            root.0.join("missing-feed.bin").display()
        )
        .as_bytes(),
    )
    .unwrap();
    let error = profile.simple_limits().unwrap_err();
    assert!(error.contains("not downgraded"), "{error}");
    assert!(!root.0.join("missing-feed.bin").exists());
    assert!(!root.0.join("store").exists());
}

#[test]
fn checked_argument_grammar_is_closed_and_requires_explicit_witness_profile() {
    let ok: Vec<String> = [
        "create-checked",
        "config",
        "submit",
        "witness",
        "--andon",
        "andon",
        "--retain-history-anchor",
        "anchor",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let parsed = arguments(&ok).unwrap();
    assert!(parsed.create && parsed.publication.is_some());
    let bad: Vec<String> = [
        "create-checked",
        "config",
        "submit",
        "--andon",
        "andon",
        "--retain-history-anchor",
        "anchor",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert!(arguments(&bad).is_err());
}
