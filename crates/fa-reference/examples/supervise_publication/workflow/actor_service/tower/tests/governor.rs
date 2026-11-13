use super::*;

fn selected() -> Vec<String> {
    [
        "serve-create",
        "config",
        "actor",
        "reviewer",
        FLAG,
        "41",
        "--requests",
        "7,8",
        "--operator-profile",
        "operator",
        "--governor-profile",
        "governor",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[test]
fn explicit_independent_governor_selects_only_a_new_plain_scheduled_tower_owner() {
    let args = selected();
    let parsed = Arguments::parse(&args).unwrap();
    assert_eq!(parsed.governor, Some(Path::new("governor")));
    assert_eq!(parsed.operator, Some(Path::new("operator")));
    assert_eq!(parsed.requests.as_deref(), Some([7, 8].as_slice()));
    assert_eq!(parsed.generation, 41);
    assert!(!parsed.open);
    let mut reordered = args[..4].to_vec();
    for pair in args[4..].chunks_exact(2).rev() {
        reordered.extend_from_slice(pair);
    }
    assert!(Arguments::parse(&reordered).is_ok());
}

#[test]
fn governor_selection_cannot_downgrade_recovery_topology_or_actor_only_grammars() {
    let args = selected();
    let mut invalid = Vec::new();
    for mode in [
        "serve-open",
        "serve-open-checked",
        "create-generated",
        "actor-submit",
    ] {
        let mut changed = args.clone();
        changed[0] = mode.into();
        if mode.ends_with("-checked") {
            changed.insert(4, "witness".into());
        }
        invalid.push(changed);
    }
    for flag in [
        "--topology-profile",
        "--precheck-profile",
        "--guarded",
        "--native-text",
    ] {
        let mut changed = args.clone();
        changed.extend([flag.into(), "selected".into()]);
        invalid.push(changed);
    }
    for missing in [FLAG, "--requests", "--operator-profile"] {
        let mut changed = args.clone();
        let at = changed.iter().position(|arg| arg == missing).unwrap();
        changed.drain(at..at + 2);
        invalid.push(changed);
    }
    let mut singleton = args.clone();
    singleton[7] = "7".into();
    invalid.push(singleton);
    let mut duplicate = args;
    duplicate.extend(["--governor-profile".into(), "another-governor".into()]);
    invalid.push(duplicate);
    for args in invalid {
        assert!(Arguments::parse(&args).is_err(), "{args:?}");
        let failure = command_with_clock(&args, None, || {
            panic!("invalid governor selection observed time")
        });
        assert!(failure.is_err());
    }
}

#[test]
fn checked_governor_requires_its_explicit_witness_profile_and_retains_closed_options() {
    let mut args = selected();
    args[0] = "serve-create-checked".into();
    args.insert(4, "witness".into());
    let selected = Arguments::parse(&args).unwrap();
    assert_eq!(selected.publication, Some(Path::new("witness")));
    assert_eq!(selected.governor, Some(Path::new("governor")));
    assert_eq!(selected.operator, Some(Path::new("operator")));
    assert!(!selected.open);
    let mut reordered = args[..5].to_vec();
    for pair in args[5..].chunks_exact(2).rev() {
        reordered.extend_from_slice(pair);
    }
    assert_eq!(
        Arguments::parse(&reordered).unwrap().publication,
        Some(Path::new("witness"))
    );
    for broken in 0..4 {
        let mut changed = args.clone();
        match broken {
            0 => {
                changed.remove(4);
            }
            1 => changed[4] = "--operator-profile".into(),
            2 => changed.extend(["--topology-profile".into(), "topology".into()]),
            _ => changed.extend(["--precheck-profile".into(), "precheck".into()]),
        }
        assert!(Arguments::parse(&changed).is_err(), "{changed:?}");
        assert!(
            command_with_clock(&changed, None, || panic!(
                "invalid checked grammar observed time"
            ))
            .is_err()
        );
    }
}

#[test]
fn governed_recovery_requires_guarded_operator_and_schedule_in_both_open_modes() {
    for checked in [false, true] {
        let mut args = selected();
        args[0] = if checked {
            "serve-open-checked"
        } else {
            "serve-open"
        }
        .into();
        if checked {
            args.insert(4, "witness".into());
        }
        args.extend(["--guarded".into(), "recovery".into()]);
        let positional = if checked { 5 } else { 4 };
        let mut reversed = args[..positional].to_vec();
        for pair in args[positional..].chunks_exact(2).rev() {
            reversed.extend_from_slice(pair);
        }
        for ordered in [&args, &reversed] {
            let selected = Arguments::parse(ordered).unwrap();
            assert!(selected.open);
            assert_eq!(selected.publication, checked.then(|| Path::new("witness")));
            assert_eq!(selected.guarded, Some(Path::new("recovery")));
            assert_eq!(selected.governor, Some(Path::new("governor")));
            assert_eq!(selected.operator, Some(Path::new("operator")));
            assert_eq!(selected.requests.as_deref(), Some([7, 8].as_slice()));
            assert!(selected.topology.is_none());
            assert!(selected.precheck.is_none());
        }
        let mut invalid = Vec::new();
        for missing in [
            "--guarded",
            "--governor-profile",
            "--operator-profile",
            "--requests",
        ] {
            let mut changed = args.clone();
            let index = changed.iter().position(|value| value == missing).unwrap();
            changed.drain(index..index + 2);
            invalid.push(changed);
        }
        for extra in ["--topology-profile", "--precheck-profile", "--guarded"] {
            let mut changed = args.clone();
            changed.extend([extra.into(), "other".into()]);
            invalid.push(changed);
        }
        let mut singleton = args.clone();
        let index = singleton
            .iter()
            .position(|value| value == "--requests")
            .unwrap();
        singleton[index + 1] = "7".into();
        invalid.push(singleton);
        let mut create = args;
        create[0] = if checked {
            "serve-create-checked"
        } else {
            "serve-create"
        }
        .into();
        invalid.push(create);
        for args in invalid {
            let expected = Arguments::parse(&args).err().unwrap();
            assert_eq!(
                command_with_clock(&args, None, || panic!(
                    "invalid governed recovery grammar observed time"
                ))
                .unwrap_err(),
                expected
            );
        }
    }
}

/// Real command profiles and an original guarded owner. An overflowing clock
/// supplies a positive preflight control without entering a pathname listener.
struct RecoveryCommand {
    root: std::path::PathBuf,
    args: Vec<String>,
    recovery: String,
    plain: String,
    scope: String,
    before: std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    anchor: Vec<u8>,
}

impl RecoveryCommand {
    fn new(checked: bool) -> Self {
        use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
        use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
        use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
            FileGuardSet, anchored::MAX_HISTORY_ANCHOR_BYTES,
        };
        use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_POLICY_PROMOTION_REPLAY_EVENTS;
        use fa_reference::action::consequence::oversight::actor_peer::PeerCredentials;
        use std::os::unix::{fs::DirBuilderExt, net::UnixStream};
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = crate::test_directory::create(format!(
            "fa-gov-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for name in ["actors", "reviewers"] {
            std::fs::DirBuilder::new()
                .mode(0o750)
                .create(root.join(name))
                .unwrap();
        }
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/supervised_publication.json"
        ))
        .replace(
            "/operator/publication-store",
            root.join("store").to_str().unwrap(),
        )
        .replace(
            "/operator/evidence.json",
            root.join("evidence.json").to_str().unwrap(),
        );
        std::fs::write(root.join("config.json"), &text).unwrap();
        let config = Config::decode(text.as_bytes()).unwrap();
        let scope_value = config.profile.delivery.scope;
        let scope = format!(
            r#"{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}"#,
            scope_value.tenant,
            scope_value.principal,
            scope_value.run,
            scope_value.branch,
            scope_value.authority
        );
        let (socket, _other) = UnixStream::pair().unwrap();
        let id = PeerCredentials::observe(&socket).unwrap();
        let identity = format!(
            r#"{{"uid":{},"gid":{},"pid":{}}}"#,
            id.uid(),
            id.gid(),
            id.pid()
        );
        let governor_pid = if id.pid() < i32::MAX as u32 {
            id.pid() + 1
        } else {
            id.pid() - 1
        };
        let governor_identity = format!(
            r#"{{"uid":{},"gid":{},"pid":{governor_pid}}}"#,
            id.uid(),
            id.gid()
        );
        let governor_document = format!(
            r#"{{"schema":"fa.policy-governor-service/1","scope":{scope},"socket":"{}","supervisor":{identity},"governor":{governor_identity},"candidate_limit":4,"connection_limit":4,"exchange_limit":4,"runtime_ms":15000,"poll_ms":1,"campaign_limit":1,"report_bytes":65536,"campaigns":{{"cases":16,"input_bytes":1048576,"max_campaigns":4}}}}"#,
            root.join("actors/governor.sock").display()
        );
        let governor = crate::workflow::actor_service::governor::Settings::decode(
            governor_document.as_bytes(),
        )
        .unwrap();
        let plain = format!(
            r#"{{"schema":"fa.supervised-witnesses/1","source":91,"original":"{}","current":"{}","limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":0,"role":"subject"}}]}}"#,
            root.join("original.bin").display(),
            root.join("current.bin").display()
        );
        for (name, document) in [
            (
                "actor.json",
                format!(
                    r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":7,"scope":{scope},"socket":"{}","supervisor":{identity},"actor":{identity},"candidate_limit":4,"connection_limit":4,"exchange_limit":32,"runtime_ms":15000,"poll_ms":1,"reply_ms":200}}"#,
                    root.join("actors/actor.sock").display()
                ),
            ),
            (
                "reviewer.json",
                format!(
                    r#"{{"version":1,"clock":"unix_milliseconds","scope":{scope},"reviewer_id":77,"socket_directory":"{}","supervisor":{identity},"reviewer":{identity},"candidate_limit":4,"runtime_ms":15000,"poll_ms":1}}"#,
                    root.join("reviewers").display()
                ),
            ),
            (
                "operator.json",
                format!(
                    r#"{{"schema":"fa.tower-operator-service/1","scope":{scope},"socket":"{}","supervisor":{identity},"operator":{identity},"candidate_limit":4,"connection_limit":4,"exchange_limit":32,"runtime_ms":15000,"poll_ms":1,"inspection_queries":0,"explanation_queries":0,"explanation_replay_events":0,"explanation_disclosure":"none","policy_promotion":{{"queries":2,"max_replay_events":{},"authority":"promote_approved_policy"}}}}"#,
                    root.join("actors/operator.sock").display(),
                    MAX_POLICY_PROMOTION_REPLAY_EVENTS
                ),
            ),
            ("governor.json", governor_document),
            ("witness.json", plain.clone()),
        ] {
            std::fs::write(root.join(name), document).unwrap();
        }
        let guards = FileGuardSet {
            stream: None,
            decoder: None,
            decoder_stop: None,
            source: Some(config.source_policy),
            identity: None,
            campaigns: Some(governor.campaign_requirement()),
            credential: None,
        };
        let (host, _) = if checked {
            FileOversight::create_guarded_publication_reserved(
                &config.store,
                config.profile.clone(),
                &guards,
                None,
                RecoveryReserve::terminal(),
                PublicationProfile::decode(plain.as_bytes()).unwrap().limits,
            )
        } else {
            FileOversight::create_guarded_reserved(
                &config.store,
                config.profile.clone(),
                &guards,
                None,
                RecoveryReserve::terminal(),
            )
        }
        .unwrap();
        let cut = host.inspect();
        let mut anchor = Vec::new();
        host.history_anchor()
            .unwrap()
            .write_to(&mut anchor, MAX_HISTORY_ANCHOR_BYTES)
            .unwrap();
        std::fs::write(root.join("before.anchor"), &anchor).unwrap();
        let config_json = fa_reference::strict_json::parse(
            text.as_bytes(),
            fa_reference::strict_json::Limits::default(),
        )
        .unwrap();
        let effective = crate::workflow::actor_service::governor::protocol::json(
            config_json.get("policy").unwrap(),
        );
        let recovery = format!(
            concat!(
                "{{\"schema_version\":\"fa.guarded-submit/2\",\"minimum\":{{\"journal_revision\":{},",
                "\"control_sequence\":{},\"authority_epoch\":{}}},\"history_anchor\":{:?},",
                "\"retain_history_anchor\":{:?},\"effective_policy\":{},",
                "\"campaigns\":{{\"cases\":16,\"input_bytes\":1048576,\"max_campaigns\":4}}}}"
            ),
            cut.revision,
            cut.control.sequence,
            cut.control.ledger.epoch,
            root.join("before.anchor").to_str().unwrap(),
            root.join("after.anchor").to_str().unwrap(),
            effective
        );
        std::fs::write(root.join("recovery.json"), &recovery).unwrap();
        drop(host);
        let mut args = vec![
            if checked {
                "serve-open-checked"
            } else {
                "serve-open"
            }
            .into(),
        ];
        args.extend(
            ["config.json", "actor.json", "reviewer.json"]
                .map(|name| root.join(name).to_str().unwrap().to_owned()),
        );
        if checked {
            args.push(root.join("witness.json").to_str().unwrap().into());
        }
        args.extend([FLAG.into(), "41".into(), "--requests".into(), "7,8".into()]);
        for (flag, name) in [
            ("--operator-profile", "operator.json"),
            ("--governor-profile", "governor.json"),
            ("--guarded", "recovery.json"),
        ] {
            args.extend([flag.into(), root.join(name).to_str().unwrap().into()]);
        }
        let mut fixture = Self {
            root,
            args,
            recovery,
            plain,
            scope,
            before: Default::default(),
            anchor,
        };
        fixture.before = fixture.files();
        fixture
    }

    fn files(&self) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
        std::fs::read_dir(self.root.join("store"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file())
            .map(|path| {
                let bytes = std::fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect()
    }

    fn unchanged(&self) {
        assert_eq!(self.files(), self.before);
        assert_eq!(
            std::fs::read(self.root.join("before.anchor")).unwrap(),
            self.anchor
        );
        for name in [
            "after.anchor",
            "evidence.json",
            "original.bin",
            "current.bin",
            "missing-feed.bin",
            "missing-producer.bin",
        ] {
            assert!(!self.root.join(name).exists(), "unexpected {name}");
        }
        for name in ["actors", "reviewers"] {
            assert_eq!(std::fs::read_dir(self.root.join(name)).unwrap().count(), 0);
        }
    }
}

impl Drop for RecoveryCommand {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.root) {
            eprintln!("governor command fixture cleanup: {error}");
        }
    }
}

#[test]
fn governed_recovery_command_refuses_legacy_or_changed_campaign_custody_before_clock() {
    for checked in [false, true] {
        let fixture = RecoveryCommand::new(checked);
        let legacy = fixture
            .recovery
            .split(",\"effective_policy\":")
            .next()
            .unwrap()
            .to_owned()
            + "}";
        for document in [
            legacy.replace("fa.guarded-submit/2", "fa.guarded-submit/1"),
            fixture.recovery.replace("\"cases\":16", "\"cases\":15"),
            fixture
                .recovery
                .replace("\"input_bytes\":1048576", "\"input_bytes\":1048575"),
            fixture
                .recovery
                .replace("\"max_campaigns\":4", "\"max_campaigns\":3"),
        ] {
            std::fs::write(fixture.root.join("recovery.json"), document).unwrap();
            let error = command_with_clock(&fixture.args, None, || {
                panic!("incompatible campaign recovery reached the clock")
            })
            .unwrap_err();
            assert!(
                error.contains("fa.guarded-submit/2")
                    && error.contains("exact independently selected governor campaign limits"),
                "{error}"
            );
            fixture.unchanged();
        }
        std::fs::write(fixture.root.join("recovery.json"), &fixture.recovery).unwrap();
        let calls = std::cell::Cell::new(0);
        assert!(
            command_with_clock(&fixture.args, None, || {
                calls.set(calls.get() + 1);
                ElapsedTick(u64::MAX)
            })
            .is_err()
        );
        assert_eq!(
            calls.get(),
            1,
            "matching original custody did not pass preflight"
        );
        fixture.unchanged();
    }
}

#[test]
fn governed_recovery_command_refuses_stronger_publication_requirements_before_clock() {
    let fixture = RecoveryCommand::new(true);
    for variant in 0..4 {
        let document = match variant {
            0 | 1 => fixture.plain.replace("fa.supervised-witnesses/1", if variant == 0 { "fa.supervised-witnesses/2" } else { "fa.supervised-witnesses/4" })
                .replace("\"source\":91", &format!(r#""source":91{},"feed":{{"path":"{}","source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}}"#,
                    if variant == 1 { ",\"max_retries\":2" } else { "" }, fixture.root.join("missing-feed.bin").display())),
            2 => format!(r#"{{"schema":"fa.supervised-whole-input/2","history":"exact_current_snapshot","source":91,"producer":{{"path":"{}","scope":{}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[]}}"#,
                fixture.root.join("missing-producer.bin").display(), fixture.scope),
            _ => format!(r#"{{"schema":"fa.supervised-joint-publication/1","joint":{{"id":9,"generation":1,"minimum_safe_roots":1,"minimum_violation_roots":1,"maximum_escape_ppm":0,"maximum_false_stop_ppm":0,"max_cases":8,"max_member_outcomes":32}},"publication":{}}}"#, fixture.plain),
        };
        PublicationProfile::decode(document.as_bytes()).unwrap();
        std::fs::write(fixture.root.join("witness.json"), document).unwrap();
        let error = command_with_clock(&fixture.args, None, || {
            panic!("stronger publication recovery reached the clock")
        })
        .unwrap_err();
        assert!(
            error.contains("not downgraded"),
            "variant {variant}: {error}"
        );
        fixture.unchanged();
    }
    std::fs::write(fixture.root.join("witness.json"), &fixture.plain).unwrap();
    let calls = std::cell::Cell::new(0);
    assert!(
        command_with_clock(&fixture.args, None, || {
            calls.set(calls.get() + 1);
            ElapsedTick(u64::MAX)
        })
        .is_err()
    );
    assert_eq!(
        calls.get(),
        1,
        "original plain witness selection did not pass preflight"
    );
    fixture.unchanged();
}
