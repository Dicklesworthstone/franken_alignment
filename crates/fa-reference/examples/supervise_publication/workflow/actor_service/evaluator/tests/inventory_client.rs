//! Explicit inventory traversal uses the real original evaluator service.
//! A flush handshake makes changes between pages deterministic: a fresh page
//! must be validated and reported before another authenticated request begins.
//! Wire mutations are separate transport negatives, not native evidence.
use super::*;
use fa_reference::strict_json::Json;
use std::{
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
};

#[path = "inventory_client/wire.rs"]
mod wire;

struct ReportedPage {
    ordinal: usize,
    resume: SyncSender<()>,
}

struct CheckedOutput {
    bytes: Vec<u8>,
    flushes: usize,
    observed: SyncSender<ReportedPage>,
    fail_flush: Option<usize>,
}

impl Write for CheckedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        let (resume, receiver) = mpsc::sync_channel(1);
        self.observed
            .send(ReportedPage {
                ordinal: self.flushes,
                resume,
            })
            .map_err(|_| io::ErrorKind::BrokenPipe)?;
        receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| io::ErrorKind::TimedOut)?;
        if self.fail_flush == Some(self.flushes) {
            Err(io::ErrorKind::BrokenPipe.into())
        } else {
            Ok(())
        }
    }
}

fn args(profile: &Path, selection: &[&str]) -> Vec<String> {
    let mut result = vec![
        "evaluator-case".to_owned(),
        profile.display().to_string(),
    ];
    result.extend(selection.iter().map(|value| (*value).to_owned()));
    result
}

fn no_clock() -> Result<ElapsedTick, String> {
    panic!("inventory traversal acquired native clock authority")
}

fn release_reported(
    events: &Receiver<ReportedPage>,
    fixture: &mut Fixture,
    service: &mut Service,
    between: &mut impl FnMut(usize, &mut Fixture, &mut Service),
) {
    while let Ok(reported) = events.try_recv() {
        between(reported.ordinal, fixture, service);
        reported.resume.send(()).unwrap();
    }
}

fn native_inventory(
    fixture: &mut Fixture,
    selection: &[&str],
    fail_flush: Option<usize>,
    mut between: impl FnMut(usize, &mut Fixture, &mut Service),
) -> (Result<(), String>, Vec<u8>, Service) {
    let profile = fixture.root.0.join("inventory-client.json");
    fs::write(
        &profile,
        profile_json(
            &fixture.settings.socket,
            &fixture.actor,
            fixture.actor.supervisor,
            fixture.settings.candidates,
        )
        .replace(
            "\"runtime_ms\":10000",
            &format!("\"runtime_ms\":{}", fixture.settings.runtime_ms),
        ),
    )
    .unwrap();
    let request = args(&profile, selection);
    let mut service = fixture.start();
    let (observed, events) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        let mut output = CheckedOutput {
            bytes: Vec::new(),
            flushes: 0,
            observed,
            fail_flush,
        };
        let result = client::command(&request, &mut output);
        (result, output.bytes)
    });
    let started = Instant::now();
    while !worker.is_finished() {
        assert!(
            started.elapsed() < Duration::from_secs(12),
            "bounded inventory client did not return"
        );
        release_reported(&events, fixture, &mut service, &mut between);
        if !worker.is_finished() {
            service.checkpoint_with_clock(fixture.driver.supervisor_mut(), &mut no_clock);
            thread::sleep(Duration::from_millis(1));
        }
    }
    let (result, output) = worker.join().unwrap();
    (result, output, service)
}

fn pages(output: &[u8]) -> Vec<Json> {
    output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(super::cases_service::parsed_page)
        .collect()
}

fn value(page: &Json) -> &Json {
    page.get("knowledge").unwrap().get("value").unwrap()
}

fn number(value: &Json, key: &str) -> u64 {
    value.get(key).unwrap().as_str().unwrap().parse().unwrap()
}

fn rounds(pages: &[Json]) -> Vec<u64> {
    pages
        .iter()
        .flat_map(|page| value(page).get("cases").unwrap().as_array().unwrap())
        .map(|case| number(case, "round"))
        .collect()
}

fn assert_read_only(fixture: &Fixture, image: &[u8], pending: usize) {
    assert_eq!(fixture.image(), image);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    let host = fixture.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.credibility_report().unwrap().pending_cases, pending);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
}

#[test]
fn explicit_pending_inventory_traverses_original_pages_and_closes_the_exact_total() {
    let mut fixture = Fixture::with_case_capacity(32);
    for id in (1..=17).rev() {
        fixture.review(id);
    }
    let before = fixture.image();
    let (result, output, service) = native_inventory(
        &mut fixture,
        &["--pending", "--max-pages", "2"],
        None,
        |_, _, _| {},
    );
    result.unwrap();
    let pages = pages(&output);
    assert_eq!(pages.len(), 2);
    assert_eq!(rounds(&pages), (101..=117).collect::<Vec<_>>());
    let first = value(&pages[0]);
    let last = value(&pages[1]);
    assert_eq!(number(first, "total_pending"), 17);
    assert_eq!(number(last, "total_pending"), 17);
    assert_eq!(number(first, "next_after_round"), 116);
    assert_eq!(number(last, "after_round"), 116);
    assert_eq!(first.get("complete").unwrap().as_bool(), Some(false));
    assert_eq!(last.get("complete").unwrap().as_bool(), Some(true));
    assert!(last.get("next_after_round").unwrap().is_null());
    assert_eq!(service.candidates, 2);
    assert!(service.transferred <= service.max_bytes);
    assert!(service.io_calls <= MAX_IO_CALLS);
    assert_read_only(&fixture, &before, 17);
}

#[test]
fn explicit_all_case_inventory_retains_one_cut_and_exact_remaining_count() {
    let mut fixture = Fixture::new();
    for id in [3, 1, 2] {
        fixture.review(id);
    }
    let before = fixture.image();
    let (result, output, service) = native_inventory(
        &mut fixture,
        &["--list", "1", "--max-pages", "3"],
        None,
        |_, _, _| {},
    );
    result.unwrap();
    let pages = pages(&output);
    assert_eq!(pages.len(), 3);
    assert_eq!(rounds(&pages), vec![101, 102, 103]);
    let basis = pages[0].get("knowledge").unwrap().get("basis").unwrap();
    for (index, page) in pages.iter().enumerate() {
        assert_eq!(page.get("knowledge").unwrap().get("basis"), Some(basis));
        assert_eq!(number(value(page), "total_cases"), 3);
        assert_eq!(number(value(page), "remaining_cases"), 3 - index as u64);
    }
    assert!(value(&pages[2]).get("next_after").unwrap().is_null());
    assert_eq!(service.candidates, 3);
    assert_read_only(&fixture, &before, 3);
}

#[test]
fn empty_inventory_is_complete_without_hiding_an_unapplied_original_review() {
    for selection in [
        vec!["--pending", "--max-pages", "2"],
        vec!["--list", "1", "--max-pages", "2"],
    ] {
        let mut fixture = Fixture::new();
        credibility::begin(&mut fixture.driver.supervisor_mut().host_mut().unwrap(), 1);
        let before = fixture.image();
        let (result, output, service) =
            native_inventory(&mut fixture, &selection, None, |_, _, _| {});
        result.unwrap();
        let pages = pages(&output);
        assert_eq!(pages.len(), 1);
        assert!(rounds(&pages).is_empty());
        let unfinished = if selection[0] == "--pending" {
            "unfinished_reviews"
        } else {
            "unapplied_rounds"
        };
        assert_eq!(number(value(&pages[0]), unfinished), 1);
        assert_eq!(service.candidates, 1);
        assert_read_only(&fixture, &before, 0);
    }
}

#[test]
fn pending_inventory_allows_clock_and_unfinished_review_changes_but_keeps_its_inventory() {
    let mut fixture = Fixture::with_case_capacity(32);
    for id in 1..=17 {
        fixture.review(id);
    }
    let mut changed = None;
    let (result, output, service) = native_inventory(
        &mut fixture,
        &["--pending", "--max-pages", "2"],
        None,
        |page, fixture, _| {
            if page == 1 {
                let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
                let revision = host.revision();
                host.observe_time(revision, ElapsedTick(2)).unwrap();
                credibility::begin(&mut host, 18);
                drop(host);
                changed = Some(fixture.image());
            }
        },
    );
    result.unwrap();
    let pages = pages(&output);
    assert_eq!(pages.len(), 2);
    assert_eq!(rounds(&pages), (101..=117).collect::<Vec<_>>());
    let basis = |index: usize| pages[index].get("knowledge").unwrap().get("basis").unwrap();
    assert_eq!(number(basis(0), "evaluation_revision"), number(basis(1), "evaluation_revision"));
    assert!(number(basis(1), "journal_revision") > number(basis(0), "journal_revision"));
    assert_eq!(number(value(&pages[0]), "unfinished_reviews"), 0);
    assert_eq!(number(value(&pages[1]), "unfinished_reviews"), 1);
    assert_eq!(service.candidates, 2);
    assert_read_only(&fixture, &changed.unwrap(), 17);
}

#[test]
fn all_case_inventory_stops_at_an_original_clock_change_without_restarting() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    fixture.review(2);
    let mut changed = None;
    let (result, output, service) = native_inventory(
        &mut fixture,
        &["--list", "1", "--max-pages", "3"],
        None,
        |page, fixture, _| {
            if page == 1 {
                let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
                let revision = host.revision();
                host.observe_time(revision, ElapsedTick(2)).unwrap();
                drop(host);
                changed = Some(fixture.image());
            }
        },
    );
    assert!(result.unwrap_err().contains("incomplete"));
    let pages = pages(&output);
    assert_eq!(pages.len(), 2);
    assert_eq!(rounds(&pages[..1]), vec![101]);
    let unknown = pages[1].get("knowledge").unwrap();
    assert_eq!(unknown.get("state").unwrap().as_str(), Some("unknown"));
    assert_eq!(unknown.get("reason").unwrap().as_str(), Some("stale_basis"));
    assert!(unknown.get("value").is_none());
    assert_eq!(service.candidates, 2);
    assert_read_only(&fixture, &changed.unwrap(), 2);
}

#[test]
fn pending_inventory_stops_at_new_applied_cases_or_labels_including_before_its_cursor() {
    for label_before_cursor in [false, true] {
        let mut fixture = Fixture::with_case_capacity(32);
        for id in 1..=17 {
            fixture.review(id);
        }
        let mut changed = None;
        let (result, output, service) = native_inventory(
            &mut fixture,
            &["--pending", "--max-pages", "3"],
            None,
            |page, fixture, service| {
                if page == 1 {
                    if label_before_cursor {
                        let assessment = fixture.request(1);
                        assessment
                            .assessment
                            .apply(
                                &mut fixture.driver.supervisor_mut().host_mut().unwrap(),
                                service.evaluator.as_ref().unwrap(),
                                &mut || Ok(()),
                                &mut || Ok(ElapsedTick(20)),
                            )
                            .unwrap();
                    } else {
                        fixture.review(18);
                    }
                    changed = Some(fixture.image());
                }
            },
        );
        assert!(result.unwrap_err().contains("incomplete"));
        let pages = pages(&output);
        assert_eq!(pages.len(), 2);
        assert_eq!(rounds(&pages[..1]), (101..=116).collect::<Vec<_>>());
        let unknown = pages[1].get("knowledge").unwrap();
        assert_eq!(unknown.get("state").unwrap().as_str(), Some("unknown"));
        assert_eq!(unknown.get("reason").unwrap().as_str(), Some("evaluation_revision_changed"));
        assert!(unknown.get("value").is_none());
        assert_eq!(service.candidates, 2);
        assert_eq!(fixture.image(), changed.unwrap());
        assert_eq!(fixture.config.source.status().read_attempts, 0);
        let host = fixture.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.credibility_report().unwrap().pending_cases, if label_before_cursor { 16 } else { 18 });
        assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, u64::from(label_before_cursor));
    }
}

#[test]
fn explicit_page_and_candidate_bounds_return_partial_prefix_without_connecting_again() {
    for candidate_bound in [false, true] {
        let mut fixture = Fixture::with_case_capacity(32);
        for id in 1..=17 {
            fixture.review(id);
        }
        if candidate_bound {
            fixture.settings.candidates = 1;
        }
        let before = fixture.image();
        let maximum = if candidate_bound { "3" } else { "1" };
        let (result, output, service) = native_inventory(
            &mut fixture,
            &["--pending", "--max-pages", maximum],
            None,
            |_, _, _| {},
        );
        let error = result.unwrap_err();
        assert!(error.contains("incomplete"));
        assert!(error.contains(if candidate_bound {
            "original evaluator candidate allowance exhausted"
        } else {
            "explicit evaluator page bound reached"
        }));
        let pages = pages(&output);
        assert_eq!(pages.len(), 1);
        assert_eq!(value(&pages[0]).get("complete").unwrap().as_bool(), Some(false));
        assert_eq!(number(value(&pages[0]), "next_after_round"), 116);
        assert_eq!(service.candidates, 1);
        assert_read_only(&fixture, &before, 17);
    }
}

#[test]
fn default_single_page_remains_explicitly_partial_without_enabling_a_walk() {
    let mut fixture = Fixture::with_case_capacity(32);
    for id in 1..=17 {
        fixture.review(id);
    }
    let before = fixture.image();
    let (result, output, service) =
        native_inventory(&mut fixture, &["--pending"], None, |_, _, _| {});
    result.unwrap();
    let pages = pages(&output);
    assert_eq!(pages.len(), 1);
    assert_eq!(value(&pages[0]).get("complete").unwrap().as_bool(), Some(false));
    assert_eq!(service.candidates, 1);
    assert_read_only(&fixture, &before, 17);
}

#[test]
fn output_failure_after_the_first_page_prevents_any_next_candidate() {
    let mut fixture = Fixture::new();
    for id in 1..=3 {
        fixture.review(id);
    }
    let before = fixture.image();
    let (result, output, service) = native_inventory(
        &mut fixture,
        &["--list", "1", "--max-pages", "3"],
        Some(1),
        |_, _, _| {},
    );
    assert!(result.unwrap_err().contains("incomplete"));
    assert_eq!(rounds(&pages(&output)), vec![101]);
    assert_eq!(service.candidates, 1);
    assert_read_only(&fixture, &before, 3);
}

#[test]
fn original_stop_between_flushed_pages_ends_the_walk_without_claiming_absence() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    fixture.review(2);
    let mut stopped = None;
    let (result, output, service) = native_inventory(
        &mut fixture,
        &["--list", "1", "--max-pages", "3"],
        None,
        |page, fixture, service| {
            if page == 1 {
                let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
                let snapshot = host.inspect();
                let revision = host.revision();
                host.request_stop(
                    revision,
                    StopRequest {
                        operation: 1,
                        expected_control_sequence: snapshot.control.sequence,
                        expected_authority_epoch: snapshot.control.ledger.epoch,
                    },
                )
                .unwrap();
                drop(host);
                stopped = Some(fixture.image());
                service.checkpoint_with_clock(fixture.driver.supervisor_mut(), &mut no_clock);
                assert!(service.retired);
            }
        },
    );
    assert!(result.unwrap_err().contains("incomplete"));
    let pages = pages(&output);
    assert_eq!(pages.len(), 1);
    assert_eq!(rounds(&pages), vec![101]);
    assert!(!value(&pages[0]).get("next_after").unwrap().is_null());
    assert_eq!(service.candidates, 1);
    assert_eq!(fixture.image(), stopped.unwrap());
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn every_reconnect_reauthenticates_the_evaluator_and_spends_the_original_candidate() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    fixture.review(2);
    let before = fixture.image();
    let (result, output, service) = native_inventory(
        &mut fixture,
        &["--list", "1", "--max-pages", "3"],
        None,
        |page, _, service| {
            if page == 1 {
                // The next OS-observed peer is the same process as before;
                // a different required PID must therefore reject its bytes.
                service.settings.evaluator.pid = Some(std::process::id() + 3);
            }
        },
    );
    assert!(result.unwrap_err().contains("incomplete"));
    assert_eq!(rounds(&pages(&output)), vec![101]);
    assert_eq!(service.candidates, 2);
    assert_read_only(&fixture, &before, 2);
}

#[test]
fn time_spent_reporting_a_page_cannot_renew_the_original_client_deadline() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    fixture.review(2);
    fixture.settings.runtime_ms = 1000;
    let before = fixture.image();
    let (result, output, service) = native_inventory(
        &mut fixture,
        &["--list", "1", "--max-pages", "3"],
        None,
        |page, _, _| {
            if page == 1 {
                thread::sleep(Duration::from_millis(1100));
            }
        },
    );
    let error = result.unwrap_err();
    assert!(error.contains("incomplete"));
    assert!(error.contains("one original evaluator client lifetime/work allowance exhausted"));
    assert_eq!(rounds(&pages(&output)), vec![101]);
    assert_eq!(service.candidates, 1);
    assert_read_only(&fixture, &before, 2);
}

#[test]
fn inventory_cli_bounds_and_closed_selection_are_checked_before_profile_io() {
    let fixture = Fixture::new();
    let missing = fixture.root.0.join("missing-inventory-profile.json");
    let mut output = Vec::new();
    let missing_error =
        client::command(&args(&missing, &["--pending"]), &mut output).unwrap_err();
    for selection in [
        vec!["--pending", "--max-pages", "1"],
        vec!["--pending", "--max-pages", "32"],
        vec!["--list", "1", "--max-pages", "1"],
        vec!["--list", "16", "--max-pages", "512"],
    ] {
        assert_eq!(
            client::command(&args(&missing, &selection), &mut output).unwrap_err(),
            missing_error,
            "valid fresh inventory did not reach the same profile boundary"
        );
    }
    for invalid in ["0", "01", "-1", "+1", "1e2", "", "18446744073709551616"] {
        for selection in [
            vec!["--pending", "--max-pages", invalid],
            vec!["--list", "1", "--max-pages", invalid],
        ] {
            assert_ne!(
                client::command(&args(&missing, &selection), &mut output).unwrap_err(),
                missing_error,
                "invalid page count reached profile I/O: {invalid}"
            );
        }
    }
    for selection in [
        vec!["--pending", "--max-pages", "33"],
        vec!["--list", "1", "--max-pages", "513"],
        vec!["--pending", "--max-pages"],
        vec!["--list", "1", "--max-pages"],
        vec!["--max-pages", "2", "--pending"],
        vec!["--pending", "--max-pages", "2", "--max-pages", "3"],
        vec!["--pending", "--evaluation-revision", "1", "--after-round", "1", "--max-pages", "2"],
        vec!["--list", "1", "--after", "1", "--at-journal", "1", "--at-evaluation", "1", "--max-pages", "2"],
        vec!["--pending", "--max-pages", "2", "--after-round", "1"],
        vec!["101", "--max-pages", "2"],
    ] {
        assert_ne!(
            client::command(&args(&missing, &selection), &mut output).unwrap_err(),
            missing_error,
            "unsupported or unbounded selection reached profile I/O"
        );
    }
    let mut assessment = args(&missing, &["--pending", "--max-pages", "2"]);
    assessment[0] = "evaluator-assess".into();
    assert!(client::command(&assessment, &mut output).is_err());
    assert!(output.is_empty());
    assert!(!missing.exists());
}
