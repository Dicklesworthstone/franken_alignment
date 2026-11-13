//! Original observation packets exercise the real checked service path. Their
//! action bindings and contents do not grant committee, human or governor roles.
use super::*;
use crate::publication::PublicationProfile;
use fa_reference::ReadWitness;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{
    FileCaptureIdentity, FilePublicationCapture,
};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationInputs, FileWitnessInput,
};
use fa_reference::action::{ActionSpec, FrozenAction, VERSION};
use fa_reference::product_frontier::{
    FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};

const RECIPE: &str = r#"[{"kind":"exact_value","key":0,"role":"subject"},{"kind":"absent_key","key":1},{"kind":"range_members","start":2,"end":6}]"#;

pub(super) fn profile_document(root: &Directory) -> String {
    format!(
        r#"{{"schema":"fa.supervised-witnesses/1","source":91,"original":"{}","current":"{}","limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":{RECIPE}}}"#,
        root.0.join("original.bin").display(),
        root.0.join("current.bin").display()
    )
}

pub(super) fn capture(config: &Config, attempt: u64, epoch: u64, payload: &[u8]) -> Vec<u8> {
    let mut target = config.profile.delivery.target;
    target.expected_version = attempt;
    let action = FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: config.profile.delivery.scope,
        target: Some(target),
        payload: payload.to_vec(),
        required_witnesses: vec![ReadWitness::Exact {
            key: 7,
            value: Some(b"ok".to_vec()),
        }],
        policy_epoch: epoch,
        deadline: ElapsedTick(100000),
        units: 7,
    })
    .unwrap();
    let key = ProjectionKey {
        source: 40,
        branch: action.spec().scope.branch,
        projection: 7,
        source_epoch: 1,
    };
    let close = TrustedClosingMarker {
        key,
        final_sequence: 1,
        marker_generation: 1,
    };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers
        .accept(key, FrontierStage::Authenticated, 1)
        .unwrap();
    frontiers.record_close(close).unwrap();
    let input = FileWitnessInput::new(
        attempt,
        attempt,
        20,
        AdapterDomainInput::new(
            DomainProjection::new(40, 1, key),
            DomainClosure::Closed(close),
        ),
        [0, 2, 4]
            .into_iter()
            .map(|key| SnapshotEntry::new(key, 1, b"original".to_vec()).unwrap())
            .collect(),
        &frontiers,
    )
    .unwrap();
    FilePublicationCapture::new(
        attempt,
        FileCaptureIdentity {
            source: 91,
            generation: attempt,
        },
        &action,
        FilePublicationInputs::new(Some(input), None),
    )
    .unwrap()
    .to_bytes()
    .unwrap()
}

pub(super) fn prepare(root: &Directory, config: &Config) -> (PublicationProfile, Vec<u8>) {
    let first = capture(config, 1, 0, b"first");
    write_complete(&root.0.join("original.bin"), &first);
    write_complete(&root.0.join("current.bin"), &first);
    (
        PublicationProfile::decode(profile_document(root).as_bytes()).unwrap(),
        capture(config, 2, 1, b"second"),
    )
}

#[test]
fn stronger_governed_publication_profiles_refuse_before_clock_sources_store_or_sockets() {
    for variant in 0..7 {
        let root = Directory::new();
        let config = configured(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let operator = operator_settings(&root, &config);
        let governor = governor::Settings::decode(
            governor_document(&root, &config, std::process::id() + 1).as_bytes(),
        )
        .unwrap();
        let document = match variant {
            0 | 1 => profile_document(&root)
                .replace("fa.supervised-witnesses/1", if variant == 0 { "fa.supervised-witnesses/2" } else { "fa.supervised-witnesses/4" })
                .replace("\"source\":91", &format!(
                    r#""source":91{},"feed":{{"path":"{}","source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}}"#,
                    if variant == 1 { ",\"max_retries\":2" } else { "" },
                    root.0.join("missing-feed.bin").display()
                )),
            2..=5 => {
                let schema = match variant {
                    2 => "fa.supervised-witnesses/3",
                    3 => "fa.supervised-witnesses/5",
                    4 => "fa.supervised-whole-input/1",
                    _ => "fa.supervised-whole-input/2",
                };
                let scope = config.profile.delivery.scope;
                format!(
                    r#"{{"schema":"{schema}"{},"source":91,"producer":{{"path":"{}","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":{}}}"#,
                    if variant == 3 || variant == 5 { ",\"history\":\"exact_current_snapshot\"" } else { "" },
                    root.0.join("missing-producer.bin").display(),
                    scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
                    if variant >= 4 { "[]" } else { RECIPE }
                )
            }
            _ => format!(
                r#"{{"schema":"fa.supervised-joint-publication/1","joint":{{"id":9,"generation":1,"minimum_safe_roots":1,"minimum_violation_roots":1,"maximum_escape_ppm":0,"maximum_false_stop_ppm":0,"max_cases":8,"max_member_outcomes":32}},"publication":{}}}"#,
                profile_document(&root)
            ),
        };
        let publication = PublicationProfile::decode(document.as_bytes()).unwrap();
        let store = config.store.clone();
        let error = service::serve_tower_governed_checked(
            config,
            &actor,
            &reviewer,
            Some(&publication),
            &[7, 8],
            GENERATION,
            &operator,
            &governor,
            || panic!("unsupported governed composition observed time"),
        )
        .unwrap_err();
        assert!(
            error.contains("not downgraded"),
            "variant {variant}: {error}"
        );
        assert!(!store.exists());
        for path in [
            actor.socket,
            operator.socket,
            governor.socket,
            reviewer.socket(7),
            reviewer.socket(8),
            root.0.join("evidence.json"),
            root.0.join("original.bin"),
            root.0.join("current.bin"),
            root.0.join("missing-feed.bin"),
            root.0.join("missing-producer.bin"),
        ] {
            assert!(!path.exists(), "refused profile touched {}", path.display());
        }
    }
}
