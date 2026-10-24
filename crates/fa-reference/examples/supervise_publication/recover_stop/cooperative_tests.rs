//! Original command fixtures with deterministic elapsed checkpoints, not sleeps.
use super::super::cooperative::run_with_elapsed;
use super::*;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn each_replay_deadline_boundary_preserves_both_files_and_emits_no_acknowledgment() {
    let root = Directory::new();
    let mut c = configured(&root);
    let host = empty_owner(&c);
    let count = host.revision() as usize;
    drop(host);
    c.programs.clear();
    let canonical = std::fs::read(c.store.join("delivery.bin")).unwrap();
    let pending = c.store.join("delivery.pending");
    std::fs::write(&pending, b"staged").unwrap();
    // Before open, before each event, and after all events at Ready.
    for timeout in 0..count + 2 {
        let mut calls = 0;
        let mut out = Vec::new();
        let error = run_with_elapsed(
            &c,
            900,
            || panic!("replay expiry cannot read logical time"),
            &mut || {
                let at = calls;
                calls += 1;
                Duration::from_millis(if at == timeout {
                    c.timing.runtime_ms
                } else {
                    0
                })
            },
            &mut out,
        )
        .unwrap_err();
        assert!(error.contains("replay deadline"), "{error}");
        assert!(out.is_empty());
        assert_eq!(calls, timeout + 1);
        assert_eq!(
            std::fs::read(c.store.join("delivery.bin")).unwrap(),
            canonical
        );
        assert_eq!(std::fs::read(&pending).unwrap(), b"staged");
        drop(FileOversight::begin_stopped_recovery(&c.store, c.profile.clone()).unwrap());
    }
    let mut out = Vec::new();
    run_with_elapsed(
        &c,
        900,
        || ElapsedTick(1000),
        &mut || Duration::ZERO,
        &mut out,
    )
    .unwrap();
    assert_eq!(
        output(&out).get("status").unwrap().as_str(),
        Some("stopped_drained")
    );
    assert!(!pending.exists());
}

#[test]
fn one_cumulative_budget_has_exact_neighbors_and_the_lock_never_gaps() {
    // Keep the original Ready/before-clock cuts and include the final
    // post-acquisition check. The exact neighbor may acquire time but must
    // still refuse before a native terminal replacement is acknowledged.
    for extra in 1..=4 {
        let root = Directory::new();
        let mut c = configured(&root);
        let host = empty_owner(&c);
        let count = host.revision();
        drop(host);
        let canonical = std::fs::read(c.store.join("delivery.bin")).unwrap();
        let staged = c.store.join("delivery.pending");
        std::fs::write(&staged, b"preserve before completion").unwrap();
        c.timing.runtime_ms = count + extra;
        let succeeds = extra == 4;
        let mut calls = 0;
        let logical = Cell::new(0);
        let mut out = Vec::new();
        let result = run_with_elapsed(
            &c,
            900,
            || {
                logical.set(logical.get() + 1);
                ElapsedTick(1000)
            },
            &mut || {
                let elapsed = calls;
                calls += 1;
                if elapsed != 0 {
                    assert!(matches!(
                        FileOversight::begin_stopped_recovery(&c.store, c.profile.clone()),
                        Err(JournalError::Busy)
                    ));
                }
                Duration::from_millis(elapsed)
            },
            &mut out,
        );
        assert_eq!(result.is_ok(), succeeds, "cut {extra}: {result:?}");
        assert_eq!(calls, (count + extra + 1).min(count + 4));
        assert_eq!(logical.get(), usize::from(extra >= 3));
        assert_eq!(out.is_empty(), !succeeds);
        if !succeeds {
            assert_eq!(
                std::fs::read(c.store.join("delivery.bin")).unwrap(),
                canonical
            );
            assert_eq!(
                std::fs::read(&staged).unwrap(),
                b"preserve before completion"
            );
        } else {
            assert_eq!(
                output(&out).get("status").unwrap().as_str(),
                Some("stopped_drained")
            );
            assert!(!staged.exists());
        }
    }
}

#[test]
fn replacement_at_ready_is_not_overwritten_or_acknowledged() {
    let root = Directory::new();
    let c = configured(&root);
    let mut host = empty_owner(&c);
    let count = host.revision() as usize;
    let original = std::fs::read(c.store.join("delivery.bin")).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000))
        .unwrap();
    let successor = std::fs::read(c.store.join("delivery.bin")).unwrap();
    drop(host);
    // Both images are actual original journals; the fixture restores the first
    // before freezing replay, then simulates out-of-band replacement at Ready.
    std::fs::write(c.store.join("delivery.bin"), &original).unwrap();
    let pending = c.store.join("delivery.pending");
    std::fs::write(&pending, b"staged").unwrap();
    let mut calls = 0;
    let mut out = Vec::new();
    let error = run_with_elapsed(
        &c,
        900,
        || panic!("canonical identity precedes clock"),
        &mut || {
            let at = calls;
            calls += 1;
            if at == count + 1 {
                std::fs::write(c.store.join("delivery.bin"), &successor).unwrap();
            }
            Duration::ZERO
        },
        &mut out,
    )
    .unwrap_err();
    assert!(error.contains("Binding"));
    assert!(out.is_empty());
    assert_eq!(
        std::fs::read(c.store.join("delivery.bin")).unwrap(),
        successor
    );
    assert_eq!(std::fs::read(&pending).unwrap(), b"staged");
}

#[test]
fn command_recovers_at_full_capacity_and_retries_without_time_or_live_assets() {
    let root = Directory::new();
    let mut c = configured(&root);
    c.profile.delivery.limits.events = 5;
    c.programs.clear();
    drop(empty_owner(&c));
    let mut out = Vec::new();
    run(&c, 900, || ElapsedTick(1000), &mut out).unwrap();
    let first = output(&out);
    assert_eq!(first.get("revision").unwrap().as_str(), Some("5"));
    let canonical = std::fs::read(c.store.join("delivery.bin")).unwrap();
    let mut calls = 0;
    out.clear();
    run_with_elapsed(
        &c,
        900,
        || panic!("completed retry"),
        &mut || {
            calls += 1;
            Duration::ZERO
        },
        &mut out,
    )
    .unwrap();
    assert_eq!(calls, 7, "five events and initial/Ready boundaries");
    assert_eq!(
        output(&out).get("recovery").unwrap().as_str(),
        Some("already_drained")
    );
    assert_eq!(
        std::fs::read(c.store.join("delivery.bin")).unwrap(),
        canonical
    );
    assert!(!root.0.join("evidence.json").exists());
}

#[test]
fn real_executed_and_unknown_publications_keep_their_original_receipt_accounting() {
    for unknown in [false, true] {
        let root = Directory::new();
        let (c, tick) = publish(&root, unknown);
        let before = FileOversight::read_publication(&c.store, &c.profile).unwrap();
        let operation = before
            .stop
            .as_ref()
            .map_or(900, |receipt| receipt.request().operation);
        let mut calls = 0;
        let mut out = Vec::new();
        let result = run_with_elapsed(
            &c,
            operation,
            || ElapsedTick(tick),
            &mut || {
                calls += 1;
                Duration::ZERO
            },
            &mut out,
        );
        assert_eq!(calls, before.revision + 4);
        assert_eq!(result.is_ok(), !unknown);
        let json = output(&out);
        assert_eq!(json.get("stop_acknowledged").unwrap().as_bool(), Some(true));
        assert_eq!(json.get("charged").unwrap().as_str(), Some("4"));
        assert_eq!(json.get("drained").unwrap().as_bool(), Some(!unknown));
        assert_eq!(
            json.get("executions").unwrap().as_str(),
            Some(if unknown { "0" } else { "1" })
        );
        assert!(c.programs.is_empty());
        assert!(!root.0.join("evidence.json").exists());
    }
}

#[test]
fn elapsed_and_terminal_clock_unwinds_release_custody_without_claiming_a_stop() {
    for terminal in [false, true] {
        let root = Directory::new();
        let c = configured(&root);
        drop(empty_owner(&c));
        let canonical = std::fs::read(c.store.join("delivery.bin")).unwrap();
        let pending = c.store.join("delivery.pending");
        std::fs::write(&pending, b"staged").unwrap();
        let mut calls = 0;
        let mut out = Vec::new();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                run_with_elapsed(
                    &c,
                    900,
                    || panic!("terminal clock unavailable"),
                    &mut || {
                        calls += 1;
                        if !terminal && calls == 3 {
                            panic!("elapsed service interrupted after first event");
                        }
                        Duration::ZERO
                    },
                    &mut out,
                )
            }))
            .is_err()
        );
        assert!(out.is_empty());
        assert_eq!(
            std::fs::read(c.store.join("delivery.bin")).unwrap(),
            canonical
        );
        assert_eq!(std::fs::read(&pending).unwrap(), b"staged");
        run(&c, 900, || ElapsedTick(1000), &mut out).unwrap();
        assert_eq!(
            output(&out).get("status").unwrap().as_str(),
            Some("stopped_drained")
        );
    }
}
