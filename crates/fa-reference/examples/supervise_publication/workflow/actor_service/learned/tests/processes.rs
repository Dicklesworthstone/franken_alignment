//! Actual child sockets, learned generation, policy gates and durable publication.
//! Child ballots are deliberate protocol fixtures, not inference-quality evidence.
use super::*;
use fa_reference::action::consequence::oversight::{
    helper_client::{ClientPhase, HelperClient},
    helper_workers::HelperLimits,
};
use fa_reference::round::Verdict;
use std::os::unix::net::UnixStream;

fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub(super) fn process_recipe(root: &Root, mode: &str) -> String {
    let executable = quote(std::env::current_exe().unwrap().to_str().unwrap());
    let directory = quote(root.0.to_str().unwrap());
    let test = quote(&format!("{}::process_child", module_path!().split_once("::").unwrap().1));
    let members = ["alpha", "beta"].into_iter().map(|name| {
        let trace = quote(root.0.join(format!("{name}.input")).to_str().unwrap());
        format!(r#""{name}":{{"executable":{executable},"directory":{directory},"arguments":["--exact",{test},"--nocapture"],"environment":{{"FA_LEARNED_PUBLICATION_PROCESS":"{mode}","FA_LEARNED_PUBLICATION_MEMBER":"{name}","FA_LEARNED_PUBLICATION_TRACE":{trace}}}}}"#)
    }).collect::<Vec<_>>().join(",");
    let raw = fixture::recipe(root)
        .replace("fa.learned-publication/1", "fa.learned-publication/6")
        .replace(&format!(r#""native_roster":"{}/native-roster.json""#, root.0.display()), r#""forecast":null"#)
        .replace(r#""budget":{"rounds":1"#, r#""priority":[],"budget":{"rounds":1"#);
    let (prefix, _) = raw.split_once(r#","native_review":"#).unwrap();
    format!(r#"{prefix},"process_review":{{"rounds":[{{"id":701,"members":{{{members}}}}}],"polls":4096,"input_bytes":{},"salt_bytes":32,"starts":2}}}}"#,
        HelperLimits::default().input_bytes)
}

fn prepare_process(root: &Root, mode: &str) -> (Config, Loaded, FileHumanReviewer, FileSupervisedDriver, Wire, Deadline) {
    let mut config = configured(root);
    let path = fixture::write(root, b"allow");
    fs::write(&path, process_recipe(root, mode)).unwrap();
    // Process mode has no supervisor-native roster dependency.
    fs::remove_file(root.0.join("native-roster.json")).unwrap();
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    assert!(loaded.native.is_none() && loaded.process.is_some());
    let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let deadline = deadline();
    let mut control = Control::new(&config, 1, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &deadline, &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &deadline,
        &mut || ElapsedTick(1000)).unwrap();
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.epoch, 0);
    (config, loaded, reviewer, driver, wire, deadline)
}

#[test]
fn process_child() {
    let Ok(mode) = std::env::var("FA_LEARNED_PUBLICATION_PROCESS") else { return; };
    if mode == "exit" { return; }
    let name = std::env::var("FA_LEARNED_PUBLICATION_MEMBER").unwrap();
    let trace = std::env::var_os("FA_LEARNED_PUBLICATION_TRACE").unwrap();
    let config = Config::decode(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"))).unwrap();
    let profile = config.profile.committee.members()[&name].profile_at(0);
    let mut client = HelperClient::<UnixStream>::from_process_stdin(profile).unwrap();
    let started = Instant::now();
    let mut answered = false;
    while started.elapsed() < Duration::from_secs(15) {
        if client.step().is_err() { return; }
        if client.phase() == ClientPhase::NeedsInference && !answered {
            let input = client.input().unwrap().actual_input();
            assert!(input.part_bytes(1).unwrap().starts_with(b"FASIDE\0\x01"));
            fs::write(&trace, input.submitted_bytes()).unwrap();
            client.respond(if mode == "deny" { Verdict::Deny } else { Verdict::Allow },
                if name == "alpha" { &[19; 32] } else { &[23; 32] }).unwrap();
            answered = true;
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("original learned process helper did not finish");
}

#[test]
fn process_recipe_review_waits_for_both_keys_and_preserves_receipt_only_recovery() {
    for approve in [true, false] {
        let root = Root::new();
        let (mut config, loaded, reviewer, driver, mut wire, deadline) = prepare_process(&root, "allow");
        let mut control = Control::new(&config, 1, None).unwrap();
        let reviewed = review::run(driver, &mut config, &loaded, &reviewer,
            &mut control, &deadline, &mut || ElapsedTick(1000));
        assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
        assert!(reviewed.history.is_none() && !reviewed.stop_requested);
        let history = reviewed.process.unwrap();
        let process::ProcessHistory::Review(process_review) = &history else { panic!("missing process history"); };
        for (name, input) in process_review.input().views() {
            assert_eq!(fs::read(root.0.join(format!("{name}.input"))).unwrap(),
                input.actual_input().submitted_bytes());
        }
        assert_eq!(process_review.admitted_starts(), 2);
        let mut driver = reviewed.driver;
        assert_eq!(driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
        assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::AwaitingHuman { request: 1 }));
        let offer = driver.request_learned_human_approval_from_policy_file(&mut config.source,
            702, ElapsedTick(50_000), || ElapsedTick(1000)).result.unwrap();
        if approve {
            let human = {
                let mut host = driver.supervisor_mut().host_mut().unwrap();
                let revision = host.revision();
                reviewer.approve(&mut host, revision, &offer).unwrap()
            };
            assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                || ElapsedTick(1000), Some(&human)).result.unwrap(), FileDriverEvent::Dispatched { .. }));
            assert!(!executed(&mut wire, 1));
            assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::PublicationChecked { .. }));
            assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::Reconciled { .. }));
        } else {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            reviewer.reject(&mut host, revision, &offer).unwrap();
            let revision = host.revision();
            host.cancel_request(revision, 1).unwrap();
        }
        assert_eq!(executed(&mut wire, 1), approve);
        let state = driver.supervisor().host().unwrap().inspect();
        assert_eq!(state.executions, u64::from(approve));
        assert_eq!(state.payload, if approve { b"A".to_vec() } else { Vec::new() });
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(cleanup(driver, 1000, 1), 0);
        assert_eq!(history.cleanup(1000, 1), 0);
        // Missing process executables and policy evidence cannot become live
        // dependencies merely to inspect an already recorded receipt.
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        let path = root.0.join("recipe.json");
        let recipe = fs::read_to_string(&path).unwrap().replace(
            &quote(std::env::current_exe().unwrap().to_str().unwrap()), "\"/unavailable/learned-review-helper\"");
        fs::write(&path, recipe).unwrap();
        let loaded = recipe::load(&path, &config, false).unwrap();
        let result = recovery::resume(config, loaded, || panic!("receipt-only recovery acquired time")).unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure);
        assert_eq!(matches!(result.response.result,
            Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })), approve);
    }
}

#[test]
fn process_deny_and_early_exit_never_create_a_publication_key() {
    for mode in ["deny", "exit"] {
        let root = Root::new();
        let (mut config, loaded, reviewer, driver, mut wire, deadline) = prepare_process(&root, mode);
        let mut control = Control::new(&config, 1, None).unwrap();
        let reviewed = review::run(driver, &mut config, &loaded, &reviewer,
            &mut control, &deadline, &mut || ElapsedTick(1000));
        let mut driver = reviewed.driver;
        assert!(!matches!(driver.phase(), FileDriverPhase::AwaitingDispatch { .. }));
        assert!(driver.request_learned_human_approval_from_policy_file(&mut config.source,
            702, ElapsedTick(50_000), || ElapsedTick(1000)).result.is_err());
        assert!(!executed(&mut wire, 1));
        assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
        assert!(driver.supervisor().host().unwrap().inspect().payload.is_empty());
        assert_eq!(cleanup(driver, 1000, 1), 0);
        assert_eq!(reviewed.process.unwrap().cleanup(1000, 1), 0);
    }
}

#[test]
fn closed_process_recipe_rejects_missing_members_rounds_and_unbounded_starts_before_assets() {
    let root = Root::new();
    let config = configured(&root);
    let valid = process_recipe(&root, "allow");
    // No model assets exist here. A valid recipe reaches their read; malformed
    // process admission must fail first with its own exact structural boundary.
    let path = root.0.join("recipe.json");
    for (document, expected) in [
        (valid.replace("\"beta\":{", "\"different\":{"), "exact independently configured congress"),
        (valid.replace("\"starts\":2", "\"starts\":1"), "complete-schedule start bounds"),
        (valid.replace("\"id\":701", "\"id\":0"), "nonzero and unique"),
        (valid.replace("\"forecast\":null", "\"unsupported_forecast\":null"), "missing learned recipe field forecast"),
        (valid.replace("\"process_review\":", "\"native_review\":"), "missing learned recipe field process_review"),
    ] {
        fs::write(&path, document).unwrap();
        let error = recipe::load(&path, &config, true).err().expect("malformed process recipe");
        assert!(error.contains(expected), "{error}");
        assert!(!config.store.exists());
    }
    fs::write(&path, valid).unwrap();
    let error = recipe::load(&path, &config, false).err().expect("no generation assets");
    assert!(!error.contains("process") && !error.contains("congress"), "{error}");
    assert!(!config.store.exists());
}
