//! Actual native review/assessment and authenticated sockets on one surviving
//! owner. These are authored regressions; execution still requires RCH.
use super::super::{Config, PeerProfile};
use super::*;
use fa_reference::action::consequence::delivery::persistent::Reconciliation;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, credibility::andon::FileAndonConfig,
};
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::oversight::{
    actor::Knowledge,
    actor_peer::{PeerCredentials, PeerPolicy},
};
use fa_reference::round::{
    Verdict,
    review::andon::{AndonBudget, LedgerIdentity},
};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[path = "../../../../../tests/support/file_credibility.rs"]
mod credibility;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(crate::test_directory::create(format!(
            "fa-live-eval-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        )))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    driver: FileSupervisedDriver,
    human: FileHumanReviewer,
    evaluator: Option<FileIndependentEvaluator>,
    settings: Settings,
    actor: Profile,
    config: Config,
    root: Directory,
}
impl Fixture {
    fn new() -> Self {
        Self::with_case_capacity(8)
    }

    fn with_case_capacity(capacity: usize) -> Self {
        let root = Directory::new();
        let (socket, _) = UnixStream::pair().unwrap();
        let credentials = PeerCredentials::observe(&socket).unwrap();
        let identity = PeerPolicy::new(
            credentials.uid(),
            credentials.gid(),
            Some(credentials.pid()),
        )
        .unwrap();
        let other = PeerPolicy::new(
            credentials.uid(),
            credentials.gid(),
            Some(credentials.pid() + 1),
        )
        .unwrap();
        let mut config = Config::decode(include_bytes!(
            "../../../../../fixtures/supervised_publication.json"
        ))
        .unwrap();
        config.store = root.0.join("store");
        config.profile = credibility::profile();
        config.profile.delivery.max_attempts = capacity;
        if capacity > 8 {
            config.profile.delivery.total = capacity as u64 * 16;
        }
        config.profile.delivery.clock_domain = crate::config::CLOCK_DOMAIN;
        let actor = Profile {
            request: 7,
            scope: config.profile.delivery.scope,
            socket: root.0.join("actor.sock"),
            supervisor: identity,
            actor: other,
            candidates: 8,
            connections: 8,
            exchanges: 128,
            runtime_ms: 10_000,
            poll_ms: 1,
            reply_ms: 50,
        };
        let settings = Settings::decode(
            profile_json(&root.0.join("eval.sock"), &actor, identity, 8).as_bytes(),
        )
        .unwrap();
        let (mut host, human) =
            FileOversight::create(&config.store, config.profile.clone()).unwrap();
        let mut protocol = credibility::protocol();
        protocol.false_stop_budget = 2;
        let evaluator = host.enable_credibility(host.revision(), protocol).unwrap();
        host.enable_andon(
            host.revision(),
            &FileAndonConfig::new(
                LedgerIdentity {
                    deployment: 101,
                    evaluator: 202,
                },
                vec![AndonBudget {
                    window: 1,
                    start: 0,
                    end: 10_000,
                    false_stops_allowed: 2,
                    cost_per_stop: u64::MAX,
                }],
            )
            .unwrap(),
        )
        .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let (_, driver) = host.into_supervised_driver();
        Self {
            driver,
            human,
            evaluator: Some(evaluator),
            settings,
            actor,
            config,
            root,
        }
    }

    fn start(&mut self) -> Service {
        Service::start(
            &self.settings,
            &self.actor,
            self.evaluator.take().unwrap(),
            &self.driver,
            &deadline(),
        )
        .unwrap()
    }

    fn review(&mut self, id: u64) {
        credibility::review(
            &mut self.driver.supervisor_mut().host_mut().unwrap(),
            id,
            Verdict::Hold,
        );
    }

    fn request(&self, id: u64) -> Request {
        let host = self.driver.supervisor().host().unwrap();
        let ticket = host.evaluation_ticket(id + 100).unwrap();
        let case = ticket.case();
        let root: String = case
            .evidence_root
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let document = format!(
            r#"{{"schema_version":"fa.native-assessment/1","round":{},"attempt":{},"policy_generation":{},"congress_generation":{},"case_evidence_root_hex":"{root}","expected_evaluation_revision":{},"origin":{id},"evidence_id_hex":"{}","truth":"benign"}}"#,
            case.round,
            case.attempt,
            case.policy_generation,
            case.congress_generation,
            host.credibility_report().unwrap().revision,
            "13".repeat(32),
        );
        Request::prepare(&self.settings, document.as_bytes()).unwrap()
    }

    fn image(&self) -> Vec<u8> {
        fs::read(self.config.store.join("delivery.bin")).unwrap()
    }

    fn exchange(
        &mut self,
        service: &mut Service,
        bytes: &[u8],
        clock: &mut impl FnMut() -> Result<ElapsedTick, String>,
    ) -> Vec<u8> {
        let mut socket = UnixStream::connect(&self.settings.socket).unwrap();
        socket.write_all(bytes).unwrap();
        socket.shutdown(Shutdown::Write).unwrap();
        socket.set_nonblocking(true).unwrap();
        let mut output = Vec::new();
        for _ in 0..256 {
            service.checkpoint_with_clock(self.driver.supervisor_mut(), clock);
            let mut buffer = [0_u8; 4096];
            loop {
                match socket.read(&mut buffer) {
                    Ok(0) => return output,
                    Ok(count) => output.extend_from_slice(&buffer[..count]),
                    Err(error) if transient(&error) => break,
                    Err(error) if error.kind() == io::ErrorKind::ConnectionReset => return output,
                    Err(error) => panic!("read evaluator fixture: {error}"),
                }
            }
        }
        panic!("bounded evaluator fixture did not finish its response")
    }
}

fn deadline() -> Deadline {
    Deadline {
        logical: ElapsedTick(10_000),
        started: Instant::now(),
        wall: Duration::from_secs(10),
    }
}

fn profile_json(socket: &Path, actor: &Profile, evaluator: PeerPolicy, candidates: u32) -> String {
    let scope = actor.scope;
    let pid = |identity: PeerPolicy| {
        identity
            .pid()
            .map_or_else(|| "null".to_owned(), |pid| pid.to_string())
    };
    format!(
        r#"{{"schema_version":"fa.native-evaluator-peer/1","socket":"{}","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"deployment":101,"evaluator":202,"supervisor":{{"uid":{},"gid":{},"pid":{}}},"peer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":{candidates},"runtime_ms":10000,"poll_ms":1}}"#,
        socket.display(),
        scope.tenant,
        scope.principal,
        scope.run,
        scope.branch,
        scope.authority,
        actor.supervisor.uid(),
        actor.supervisor.gid(),
        pid(actor.supervisor),
        evaluator.uid(),
        evaluator.gid(),
        pid(evaluator),
    )
}

#[test]
fn authenticated_labels_change_original_live_ledger_and_later_publication_still_needs_both_keys() {
    let mut fixture = Fixture::new();
    let (ledger, grant) = fixture
        .driver
        .supervisor()
        .governance_ledger_port(3)
        .unwrap();
    let mut service = fixture.start();
    let mut first = None;
    for id in [1, 2] {
        fixture.review(id);
        let request = fixture.request(id);
        let clock_calls = Cell::new(0);
        let reply = fixture.exchange(&mut service, &request.wire, &mut || {
            clock_calls.set(clock_calls.get() + 1);
            Ok(ElapsedTick(id * 20))
        });
        assert_eq!(
            protocol::validate(&request, &reply).unwrap(),
            Outcome::Applied
        );
        assert_eq!(clock_calls.get(), 1);
        let before = fixture.image();
        let Knowledge::Known {
            value: snapshot, ..
        } = ledger.ledger().unwrap()
        else {
            panic!("expected original accounting")
        };
        assert_eq!(snapshot.credibility().unwrap().lifetime_false_stops(), id);
        let accounting = &snapshot.andon().unwrap().windows()[&1];
        assert_eq!(accounting.false_stops, id);
        assert_eq!(accounting.total_cost, u128::from(id) * u128::from(u64::MAX));
        assert_eq!(accounting.incident.is_some(), id == 2);
        assert_eq!(before, fixture.image());
        if id == 1 {
            first = Some(request);
        }
    }
    // The global expected revision is now stale, but this EXACT original label
    // still uses the same native ticket and appends no Time or second charge.
    let original = first.unwrap();
    let before = fixture.image();
    let reply = fixture.exchange(&mut service, &original.wire, &mut || {
        panic!("exact retry acquired time")
    });
    assert_eq!(
        protocol::validate(&original, &reply).unwrap(),
        Outcome::AlreadyApplied
    );
    assert_eq!(fixture.image(), before);
    assert_eq!(service.candidates, 3);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        assert_eq!(host.inspect().payload, b"initial");
        let keys = credibility::ordinary::ready(
            &mut host,
            &fixture.human,
            3,
            b"later reviewed publication",
        );
        let revision = host.revision();
        assert!(host.publish(revision, 3).is_err());
        credibility::ordinary::dispatch(&mut host, &keys);
        let revision = host.revision();
        let outcome = host
            .publish_checked(
                revision,
                3,
                Some(&keys.inputs),
                credibility::snapshot(),
                ElapsedTick(40),
            )
            .unwrap()
            .outcome;
        assert_eq!(
            outcome,
            EndpointOutcome::Executed {
                resulting_version: 2
            }
        );
        let revision = host.revision();
        assert_eq!(
            host.reconcile(revision, 3).unwrap(),
            Reconciliation::Resolved(outcome),
        );
        assert_eq!(host.inspect().payload, b"later reviewed publication");
        assert_eq!(host.inspect().executions, 1);
    }
    let Knowledge::Known {
        value: snapshot, ..
    } = ledger.ledger().unwrap()
    else {
        panic!("expected original ledger")
    };
    assert_eq!(snapshot.andon().unwrap().windows()[&1].false_stops, 2);
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn wrong_audience_case_and_revision_never_acquire_clock_or_change_original_bytes() {
    for changed in ["audience", "case", "revision"] {
        let mut fixture = Fixture::new();
        fixture.review(1);
        let request = fixture.request(1);
        let mut service = fixture.start();
        let before = fixture.image();
        let wire = if changed == "audience" {
            String::from_utf8(request.wire.clone())
                .unwrap()
                .replacen("\"deployment\":101", "\"deployment\":999", 1)
                .into_bytes()
        } else {
            let encoded = request.parsed.as_object().unwrap()["assessment_hex"]
                .as_str()
                .unwrap();
            let original = String::from_utf8(crate::config::hex(encoded).unwrap()).unwrap();
            let document = if changed == "case" {
                original.replace(
                    &format!("\"attempt\":{}", request.assessment.attempt),
                    &format!("\"attempt\":{}", request.assessment.attempt + 1),
                )
            } else {
                original.replace(
                    &format!(
                        "\"expected_evaluation_revision\":{}",
                        request.assessment.expected_revision
                    ),
                    &format!(
                        "\"expected_evaluation_revision\":{}",
                        request.assessment.expected_revision + 1
                    ),
                )
            };
            Request::prepare(&fixture.settings, document.as_bytes())
                .unwrap()
                .wire
        };
        let reply = fixture.exchange(&mut service, &wire, &mut || {
            panic!("refused binding acquired time")
        });
        if changed == "audience" {
            assert!(reply.is_empty());
        } else {
            let sent = Request::accept(&fixture.settings, &wire).unwrap();
            assert_eq!(protocol::validate(&sent, &reply).unwrap(), Outcome::Refused);
        }
        assert_eq!(fixture.image(), before);
    }
}

#[test]
fn nonbenign_labels_are_clock_free_and_censored_resolution_uses_the_same_case() {
    for truth in ["censored", "violation"] {
        let mut fixture = Fixture::new();
        fixture.review(1);
        let benign = fixture.request(1);
        let encoded = benign.parsed.as_object().unwrap()["assessment_hex"]
            .as_str()
            .unwrap();
        let document = String::from_utf8(crate::config::hex(encoded).unwrap())
            .unwrap()
            .replace("\"truth\":\"benign\"", &format!("\"truth\":\"{truth}\""));
        let request = Request::prepare(&fixture.settings, document.as_bytes()).unwrap();
        let mut service = fixture.start();
        let before = fixture.driver.supervisor().host().unwrap().revision();
        let reply = fixture.exchange(&mut service, &request.wire, &mut || {
            panic!("nonbenign assessment acquired a clock")
        });
        assert_eq!(
            protocol::validate(&request, &reply).unwrap(),
            Outcome::Applied
        );
        {
            let host = fixture.driver.supervisor().host().unwrap();
            assert_eq!(host.revision(), before + 1);
            assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
            assert_eq!(host.andon_ledger().unwrap().windows()[&1].total_cost, 0);
        }
        if truth == "censored" {
            assert_eq!(
                fixture
                    .driver
                    .supervisor()
                    .host()
                    .unwrap()
                    .credibility_report()
                    .unwrap()
                    .censored_cases,
                1
            );
            let resolved = fixture.request(1);
            let reply = fixture.exchange(&mut service, &resolved.wire, &mut || Ok(ElapsedTick(20)));
            assert_eq!(
                protocol::validate(&resolved, &reply).unwrap(),
                Outcome::Applied
            );
            let host = fixture.driver.supervisor().host().unwrap();
            assert_eq!(host.credibility_report().unwrap().censored_cases, 0);
            assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
        }
    }
}

#[test]
fn incomplete_frame_can_yield_to_stop_without_time_or_label() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let request = fixture.request(1);
    let mut service = fixture.start();
    let mut socket = UnixStream::connect(&fixture.settings.socket).unwrap();
    socket
        .write_all(&request.wire[..request.wire.len() / 2])
        .unwrap();
    let before = fixture.image();
    service.checkpoint_with_clock(fixture.driver.supervisor_mut(), &mut || {
        panic!("prefix acquired time")
    });
    assert_eq!(fixture.image(), before);
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let state = host.inspect();
        let revision = host.revision();
        host.request_stop(
            revision,
            StopRequest {
                operation: 1,
                expected_control_sequence: state.control.sequence,
                expected_authority_epoch: state.control.ledger.epoch,
            },
        )
        .unwrap();
    }
    let stopped = fixture.image();
    service.checkpoint_with_clock(fixture.driver.supervisor_mut(), &mut || {
        panic!("stopped evaluator acquired time")
    });
    assert!(service.retired);
    assert_eq!(fixture.image(), stopped);
    assert_eq!(
        fixture
            .driver
            .supervisor()
            .host()
            .unwrap()
            .credibility_report()
            .unwrap()
            .pending_cases,
        1,
    );
}

#[test]
fn truncated_frame_and_wrong_peer_spend_original_candidate_quota_across_reconnects() {
    for wrong_peer in [false, true] {
        let mut fixture = Fixture::new();
        fixture.review(1);
        fixture.settings.candidates = 2;
        if wrong_peer {
            fixture.settings.evaluator.pid = Some(std::process::id() + 3);
        }
        let request = fixture.request(1);
        let mut service = fixture.start();
        let before = fixture.image();
        for expected in [1, 2] {
            let reply = fixture.exchange(
                &mut service,
                &request.wire[..request.wire.len() - 1],
                &mut || panic!("invalid peer/frame acquired time"),
            );
            assert!(reply.is_empty());
            assert_eq!(service.candidates, expected);
        }
        service.checkpoint_with_clock(fixture.driver.supervisor_mut(), &mut || {
            panic!("exhausted evaluator acquired time")
        });
        assert!(service.retired);
        assert_eq!(fixture.image(), before);
        assert!(!fixture.settings.socket.exists());
    }
}

#[test]
fn lost_reply_and_exact_resubmission_preserve_one_original_charge() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let request = fixture.request(1);
    let mut service = fixture.start();
    let mut socket = UnixStream::connect(&fixture.settings.socket).unwrap();
    socket.write_all(&request.wire).unwrap();
    socket.shutdown(Shutdown::Write).unwrap();
    drop(socket);
    service.checkpoint_with_clock(fixture.driver.supervisor_mut(), &mut || Ok(ElapsedTick(20)));
    let before = fixture.image();
    let reply = fixture.exchange(&mut service, &request.wire, &mut || {
        panic!("lost reply caused reassessment")
    });
    assert_eq!(
        protocol::validate(&request, &reply).unwrap(),
        Outcome::AlreadyApplied
    );
    assert_eq!(fixture.image(), before);
    assert_eq!(
        fixture
            .driver
            .supervisor()
            .host()
            .unwrap()
            .andon_ledger()
            .unwrap()
            .windows()[&1]
            .false_stops,
        1,
    );
}

#[test]
fn uncertain_time_storage_returns_unconfirmed_without_a_stale_success_claim() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let request = fixture.request(1);
    let mut service = fixture.start();
    let stage = fixture.config.store.join("delivery.pending");
    let reply = fixture.exchange(&mut service, &request.wire, &mut || {
        fs::write(&stage, b"interrupted writer").unwrap();
        Ok(ElapsedTick(20))
    });
    assert_eq!(
        protocol::validate(&request, &reply).unwrap(),
        Outcome::Unconfirmed
    );
    assert!(
        fixture
            .driver
            .supervisor()
            .host()
            .unwrap()
            .storage_failure()
            .is_some()
    );
    let value = fa_reference::strict_json::parse(
        &reply,
        fa_reference::strict_json::Limits {
            max_bytes: protocol::RESPONSE_BYTES,
            max_depth: 6,
            max_items: 128,
            max_string_bytes: 16_384,
        },
    )
    .unwrap();
    assert!(value.as_object().unwrap()["after"].is_null());
}

#[test]
fn evaluator_profile_refuses_overlapping_roles_and_listener_custody_aliases() {
    let fixture = Fixture::new();
    let current = fixture.actor.supervisor;
    let reviewer_pid = current.pid().unwrap() + 2;
    let scope = fixture.actor.scope;
    let reviewer = PeerProfile::decode(format!(
        r#"{{"version":1,"clock":"unix_milliseconds","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"reviewer_id":77,"socket_directory":"{}","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{reviewer_pid}}},"candidate_limit":8,"runtime_ms":10000,"poll_ms":1}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
        fixture.root.0.display(), current.uid(), current.gid(), current.pid().unwrap(),
        current.uid(), current.gid(),
    ).as_bytes()).unwrap();
    let identity = LedgerIdentity {
        deployment: 101,
        evaluator: 202,
    };
    fixture
        .settings
        .check_host(
            &fixture.config,
            &fixture.actor,
            &reviewer,
            None,
            &[7, 8],
            identity,
        )
        .unwrap();
    for pid in [None, fixture.actor.actor.pid(), Some(reviewer_pid)] {
        let mut settings = fixture.settings.clone();
        settings.evaluator.pid = pid;
        assert!(
            settings
                .check_host(
                    &fixture.config,
                    &fixture.actor,
                    &reviewer,
                    None,
                    &[7, 8],
                    identity,
                )
                .is_err()
        );
    }
    assert!(
        fixture
            .settings
            .check_custody_paths(&[&fixture.settings.socket])
            .is_err()
    );
    let mut settings = fixture.settings.clone();
    settings.socket = reviewer.socket(8);
    assert!(
        settings
            .check_host(
                &fixture.config,
                &fixture.actor,
                &reviewer,
                None,
                &[7, 8],
                identity,
            )
            .is_err()
    );
}

#[test]
fn live_receipts_reject_cross_request_and_false_completion() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let request = fixture.request(1);
    let before = Basis {
        journal: 10,
        evaluation: request.assessment.expected_revision,
        control: 0,
        epoch: 0,
    };
    assert!(protocol::receipt(&request, Outcome::Applied, before, Some(before)).is_err());
    assert!(
        protocol::receipt(
            &request,
            Outcome::AlreadyApplied,
            before,
            Some(Basis {
                journal: 11,
                ..before
            }),
        )
        .is_err()
    );
    assert!(protocol::receipt(&request, Outcome::Applied, before, None).is_err());
    let applied = Basis {
        journal: before.journal + 2,
        evaluation: before.evaluation + 1,
        ..before
    };
    for changed in [
        Basis {
            journal: applied.journal + 1,
            ..applied
        },
        Basis {
            journal: before.journal + 1,
            ..applied
        },
        Basis {
            control: 1,
            ..applied
        },
        Basis {
            epoch: 1,
            ..applied
        },
    ] {
        assert!(protocol::receipt(&request, Outcome::Applied, before, Some(changed)).is_err());
    }
    let stale = Basis {
        evaluation: before.evaluation + 1,
        ..before
    };
    assert!(
        protocol::receipt(
            &request,
            Outcome::Applied,
            stale,
            Some(Basis {
                evaluation: stale.evaluation + 1,
                ..applied
            }),
        )
        .is_err()
    );
    let time_only = Basis {
        journal: before.journal + 1,
        ..before
    };
    assert!(protocol::receipt(&request, Outcome::Refused, before, Some(time_only)).is_ok());
    assert!(
        protocol::receipt(
            &request,
            Outcome::Refused,
            before,
            Some(Basis {
                journal: before.journal + 2,
                ..before
            }),
        )
        .is_err()
    );
    assert!(
        protocol::receipt(
            &request,
            Outcome::Refused,
            stale,
            Some(Basis {
                journal: stale.journal + 1,
                ..stale
            }),
        )
        .is_err()
    );
    let valid = protocol::receipt(&request, Outcome::Applied, before, Some(applied)).unwrap();
    let altered =
        String::from_utf8(valid)
            .unwrap()
            .replacen("\"deployment\":101", "\"deployment\":999", 1);
    assert!(protocol::validate(&request, altered.as_bytes()).is_err());
}

#[test]
fn live_client_verifies_supervisor_before_disclosing_assessment_bytes() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let request = fixture.request(1);
    let mut wrong_server = fixture.actor.clone();
    wrong_server.supervisor = PeerPolicy::new(
        fixture.actor.supervisor.uid(),
        fixture.actor.supervisor.gid(),
        Some(std::process::id() + 3),
    )
    .unwrap();
    let profile = fixture.root.0.join("evaluator.json");
    fs::write(
        &profile,
        profile_json(
            &fixture.settings.socket,
            &wrong_server,
            fixture.actor.supervisor,
            2,
        ),
    )
    .unwrap();
    let assessment = fixture.root.0.join("assessment.json");
    fs::write(
        &assessment,
        crate::config::hex(
            request.parsed.as_object().unwrap()["assessment_hex"]
                .as_str()
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let listener = std::os::unix::net::UnixListener::bind(&fixture.settings.socket).unwrap();
    let reader = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut bytes = Vec::new();
        socket.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let error = client::command(
        &[
            "evaluator-assess".to_owned(),
            profile.display().to_string(),
            assessment.display().to_string(),
        ],
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(error.contains("unverified supervisor"));
    assert!(reader.join().unwrap().is_empty());
}

#[test]
fn foreign_evaluator_refuses_shared_assessment_before_timestamping_another_owner() {
    let mut original = Fixture::new();
    let mut replacement = Fixture::new();
    original.review(1);
    replacement.review(1);
    assert_eq!(original.actor.scope, replacement.actor.scope);
    {
        let original_host = original.driver.supervisor().host().unwrap();
        let replacement_host = replacement.driver.supervisor().host().unwrap();
        assert_eq!(
            original_host.evaluation_ticket(101).unwrap().case(),
            replacement_host.evaluation_ticket(101).unwrap().case()
        );
        assert_eq!(
            original_host.credibility_report().unwrap().revision,
            replacement_host.credibility_report().unwrap().revision
        );
    }
    let request = original.request(1);
    let before = replacement.image();
    let revision = replacement.driver.supervisor().host().unwrap().revision();
    {
        let mut host = replacement.driver.supervisor_mut().host_mut().unwrap();
        assert!(
            request
                .assessment
                .apply(
                    &mut host,
                    original.evaluator.as_ref().unwrap(),
                    &mut || Ok(()),
                    &mut || panic!("foreign evaluator acquired the original Time clock"),
                )
                .is_err()
        );
        assert_eq!(host.revision(), revision);
        assert!(host.evaluation_history(101).unwrap().is_empty());
    }
    assert_eq!(replacement.image(), before);
    let expected = original.image();
    assert!(
        request
            .assessment
            .apply(
                &mut original.driver.supervisor_mut().host_mut().unwrap(),
                original.evaluator.as_ref().unwrap(),
                &mut || Ok(()),
                &mut || Ok(ElapsedTick(20)),
            )
            .unwrap()
    );
    assert_ne!(original.image(), expected);
}

#[path = "tests/case_service.rs"]
mod case_service;

#[path = "tests/case_client.rs"]
mod case_client;

#[path = "tests/cases_service.rs"]
mod cases_service;

#[path = "tests/cases_client.rs"]
mod cases_client;

#[path = "tests/pending_service.rs"]
mod pending_service;

#[path = "tests/pending_client.rs"]
mod pending_client;

#[path = "tests/inventory_client.rs"]
mod inventory_client;

#[test]
fn narrowed_actor_deadline_refuses_a_late_label_and_cannot_renew_the_original_service() {
    let mut fixture = Fixture::new();
    let mut service = fixture.start();
    let narrowed = Deadline {
        logical: ElapsedTick(10),
        started: service.started,
        wall: Duration::from_secs(5),
    };
    service.narrow_deadline(&narrowed);
    assert_eq!(service.logical, ElapsedTick(10));
    assert_eq!(service.wall, Duration::from_secs(5));
    service.narrow_deadline(&deadline());
    assert_eq!(service.logical, ElapsedTick(10));
    assert_eq!(service.wall, Duration::from_secs(5));

    fixture.review(1);
    let request = fixture.request(1);
    let image = fixture.image();
    let reply = fixture.exchange(&mut service, &request.wire, &mut || Ok(ElapsedTick(10)));
    assert_eq!(
        protocol::validate(&request, &reply).unwrap(),
        Outcome::Refused
    );
    assert_eq!(
        fixture.image(),
        image,
        "expired labeling must not append Time or truth"
    );
    let host = fixture.driver.supervisor().host().unwrap();
    assert_eq!(host.credibility_report().unwrap().pending_cases, 1);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    let spent = (service.candidates, service.io_calls, service.transferred);
    assert_eq!(spent.0, 1);
    assert!(spent.1 > 0 && spent.2 > 0);
    service.narrow_deadline(&deadline());
    assert_eq!(
        (service.candidates, service.io_calls, service.transferred),
        spent
    );
    assert_eq!(service.logical, ElapsedTick(10));
    assert_eq!(service.wall, Duration::from_secs(5));
}
