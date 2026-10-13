use super::*;
use fa_reference::action::consequence::oversight::actor_wire::{WireError, decode_response, encode_command};
use std::{cell::Cell, fs, io, os::unix::fs::PermissionsExt};
#[path = "fixture.rs"]
mod fixture;
use fixture::*;

fn executed(bytes: &[u8]) {
    assert!(bytes.ends_with(b"\n"));
    let response = decode_response(&bytes[..bytes.len() - 1]).unwrap();
    assert_eq!(response.request, Some(1));
    assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert!(!bytes.windows(6).any(|w| w == b"FAHANC"));
}

#[test]
fn recovery_profile_is_closed_bounded_and_distinct_from_stop_profiles() {
    let case = queued(); let text = profile_text(&case, None);
    ResumeProfile::decode(text.as_bytes()).unwrap();
    assert!(crate::recover_stop::guarded::Profile::decode(text.as_bytes()).is_err());
    let stop = text.replace("fa.guarded-resume/1", "fa.guarded-stop/1");
    crate::recover_stop::guarded::Profile::decode(stop.as_bytes()).unwrap();
    assert!(ResumeProfile::decode(stop.as_bytes()).is_err());
    for invalid in [
        text.replace("fa.guarded-resume/1", "fa.guarded-resume/2"),
        text.replacen('{', "{\"unexpected\":true,", 1),
        text.replacen('{', "{\"schema_version\":\"fa.guarded-resume/1\",", 1),
        text.replace(&format!("\"journal_revision\":{}", case.before.revision), "\"journal_revision\":-1"),
        text.replace(&format!("\"journal_revision\":{}", case.before.revision), "\"journal_revision\":1.5"),
        text.replace(&format!("\"journal_revision\":{}", case.before.revision), "\"journal_revision\":18446744073709551616"),
        text.replace(&format!("\"journal_revision\":{},", case.before.revision), ""),
        text.replace(case.anchor.to_str().unwrap(), "relative.anchor"),
        text.replace(case.anchor.to_str().unwrap(), &"x".repeat(4097)),
        format!("{}{}", " ".repeat(16385), text),
    ] { assert!(ResumeProfile::decode(invalid.as_bytes()).is_err()); }
    let explicit_zero = text.replace(&format!("\"journal_revision\":{}", case.before.revision), "\"journal_revision\":0")
        .replace(&format!("\"control_sequence\":{}", case.before.control.sequence), "\"control_sequence\":0")
        .replace(&format!("\"authority_epoch\":{}", case.before.control.ledger.epoch), "\"authority_epoch\":0");
    let p = ResumeProfile::decode(explicit_zero.as_bytes()).unwrap();
    assert_eq!(p.minimum().journal_revision, 0); assert_eq!(p.minimum().control_sequence, 0);
    assert_eq!(p.minimum().authority_epoch, 0);
}

#[test]
fn existing_executed_receipt_is_recovered_and_retained_without_live_assets_or_another_send() {
    let (mut case, tick) = published(false); let output = case.root.0.join("recovered.anchor");
    let previous_path = case.anchor.clone(); let previous_bytes = case.archive.clone();
    let p = profile(&case, Some(&output)); let calls = Cell::new(0); let mut out = Vec::new();
    let reads = case.config.source.status().read_attempts;
    assert!(!case.root.0.join("evidence.json").exists()); assert!(case.config.programs.is_empty());
    run(&case.config, &p, &case.document, || { calls.set(calls.get() + 1); ElapsedTick(tick) }, &mut out).unwrap();
    assert_eq!(calls.get(), 1); executed(&out);
    let recovered = state(&case); assert_eq!(recovered.revision, case.before.revision + 3);
    assert_eq!(recovered.executions, 1); assert_eq!(recovered.control.ledger.charged, 4);
    assert_eq!(recovered.payload, b"once"); assert_eq!(load(&output).revision(), recovered.revision);
    assert_eq!(fs::metadata(&output).unwrap().permissions().mode() & 0o077, 0);
    assert_eq!(fs::read(&previous_path).unwrap(), previous_bytes);
    assert_eq!(case.config.source.status().read_attempts, reads);
    // Live recovery always fences again, unlike already-drained stop recovery.
    // The original request, payload and charge remain unchanged on the new cut.
    case.before = recovered; case.anchor = output; case.archive = fs::read(&case.anchor).unwrap();
    let next = case.root.0.join("recovered-again.anchor"); let p = profile(&case, Some(&next)); out.clear();
    run(&case.config, &p, &case.document, || ElapsedTick(tick + 1), &mut out).unwrap(); executed(&out);
    assert_eq!(state(&case).revision, case.before.revision + 3); assert_eq!(state(&case).executions, 1);
    assert_eq!(state(&case).control.ledger.charged, 4); assert_eq!(load(&next).revision(), state(&case).revision);
    assert_eq!(fs::read(&case.anchor).unwrap(), case.archive); assert_eq!(fs::read(previous_path).unwrap(), previous_bytes);
}

#[test]
fn malformed_unknown_and_missing_store_requests_cannot_spend_a_fence_or_create_a_request() {
    let case = queued(); let output = case.root.0.join("unwritten.anchor"); let p = profile(&case, Some(&output));
    let before = files(&case); let mut out = Vec::new();
    for doc in [b"{}".to_vec(), encode_command(&Command::Poll { request: 1 }).unwrap(),
        document(&case.config, 999, b"once")] {
        assert!(run(&case.config, &p, &doc, || panic!("invalid receipt cannot acquire time"), &mut out).is_err());
        assert!(out.is_empty()); assert_eq!(files(&case), before); assert!(!output.exists());
    }
    let backup = case.root.0.join("temporarily-offline"); fs::rename(&case.config.store, &backup).unwrap();
    assert!(run(&case.config, &p, &case.document, || panic!("missing store"), &mut out).is_err());
    assert!(!case.config.store.exists()); assert!(!output.exists()); fs::rename(backup, &case.config.store).unwrap();
    let error = run(&case.config, &p, &case.document, || ElapsedTick(1001), &mut out).unwrap_err();
    assert!(error.contains("acknowledged")); assert!(error.contains("does not confirm execution"));
    let response = decode_response(out.strip_suffix(b"\n").unwrap()).unwrap();
    assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_eq!(state(&case).executions, 0); assert_eq!(load(&output).revision(), case.before.revision + 3);
}

#[test]
fn changed_proposal_bytes_conflict_in_the_original_gateway_and_cannot_publish_again() {
    let (case, tick) = published(false); let output = case.root.0.join("conflicted.anchor");
    let p = profile(&case, Some(&output)); let wrong = document(&case.config, 1, b"else");
    let mut out = Vec::new();
    assert!(run(&case.config, &p, &wrong, || ElapsedTick(tick), &mut out).is_err());
    let response = decode_response(out.strip_suffix(b"\n").unwrap()).unwrap();
    assert_eq!(response.result, Err(WireError::IdempotencyConflict));
    assert_eq!(state(&case).executions, 1); assert_eq!(state(&case).payload, b"once");
    assert_eq!(state(&case).revision, case.before.revision + 3);
    assert_eq!(load(&output).revision(), state(&case).revision);
}

#[test]
fn every_elapsed_checkpoint_cancels_before_mutation_including_verified_ready() {
    let mut case = queued(); case.config.timing.runtime_ms = 50;
    let output = case.root.0.join("deadline.anchor"); let p = profile(&case, Some(&output));
    let original = files(&case); let count = case.before.revision as usize;
    let wall = Duration::from_millis(50); let early = wall - Duration::from_nanos(1);
    for expire in 0..=count + 2 {
        let mut calls = 0; let mut out = Vec::new();
        let result = run_with_elapsed(&case.config, &p, &case.document,
            || panic!("cancelled replay cannot read terminal clock"),
            &mut || { let cut = calls; calls += 1; if cut == expire { wall } else { early } }, &mut out);
        assert_eq!(result.unwrap_err(), "guarded receipt recovery replay deadline reached");
        assert_eq!(calls, expire + 1); assert_eq!(files(&case), original);
        assert!(out.is_empty()); assert!(!output.exists());
    }
    let mut calls = 0; let mut out = Vec::new();
    let result = run_with_elapsed(&case.config, &p, &case.document, || ElapsedTick(1001),
        &mut || { calls += 1; early }, &mut out);
    assert!(result.unwrap_err().contains("acknowledged"));
    assert_eq!(calls, count + 3); assert_eq!(load(&output).revision(), case.before.revision + 3);
}

#[test]
fn independent_floors_source_anchor_and_output_custody_are_not_defaulted_or_downgraded() {
    let mut case = queued(); let original = files(&case); let output = case.root.0.join("valid.anchor");
    let text = profile_text(&case, Some(&output)); let wrong_floor = text.replace(
        &format!("\"journal_revision\":{}", case.before.revision), "\"journal_revision\":18446744073709551615");
    let p = ResumeProfile::decode(wrong_floor.as_bytes()).unwrap(); let mut out = Vec::new();
    assert!(run(&case.config, &p, &case.document, || panic!("wrong floor"), &mut out).is_err());
    let p = profile(&case, Some(&output)); let source = case.config.source_policy;
    case.config.source_policy.source.generation += 1;
    assert!(run(&case.config, &p, &case.document, || panic!("wrong source"), &mut out).is_err());
    case.config.source_policy = source;
    let foreign = queued(); let wrong = text.replace(case.anchor.to_str().unwrap(), foreign.anchor.to_str().unwrap());
    let p = ResumeProfile::decode(wrong.as_bytes()).unwrap();
    assert!(run(&case.config, &p, &case.document, || panic!("foreign anchor"), &mut out).is_err());
    for selected in [case.anchor.clone(), case.config.store.join("inside.anchor")] {
        let p = profile(&case, Some(&selected));
        assert!(run(&case.config, &p, &case.document, || panic!("unsafe output"), &mut out).is_err());
    }
    assert_eq!(files(&case), original); assert!(out.is_empty()); assert!(!output.exists());
    assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    let p = profile(&case, Some(&output));
    assert!(run(&case.config, &p, &case.document, || ElapsedTick(1001), &mut out).unwrap_err().contains("acknowledged"));
    assert_eq!(load(&output).revision(), case.before.revision + 3);
}

struct Broken;
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
    fn flush(&mut self) -> io::Result<()> { panic!("failed write never flushes") }
}

#[test]
fn retention_and_output_failures_preserve_the_acknowledged_receipt_and_both_errors() {
    for collide in [false, true] {
        let (case, tick) = published(false); let output = case.root.0.join("raced.anchor");
        let p = profile(&case, Some(&output));
        let error = run(&case.config, &p, &case.document, || {
            if collide { fs::write(&output, b"existing custodian").unwrap(); }
            ElapsedTick(tick)
        }, &mut Broken).unwrap_err();
        assert!(error.contains("acknowledged")); assert!(error.contains("receipt output failed"));
        assert_eq!(error.contains("history anchor retention failed"), collide);
        assert_eq!(state(&case).executions, 1); assert_eq!(state(&case).control.ledger.charged, 4);
        if collide { assert_eq!(fs::read(&output).unwrap(), b"existing custodian"); }
        else { assert_eq!(load(&output).revision(), state(&case).revision); }
        let corrected = case.root.0.join("corrected.anchor"); let p = profile(&case, Some(&corrected));
        let mut out = Vec::new(); run(&case.config, &p, &case.document, || ElapsedTick(tick + 1), &mut out).unwrap();
        executed(&out); assert_eq!(state(&case).executions, 1);
        assert_eq!(load(&corrected).revision(), state(&case).revision);
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    }
}

#[test]
fn expired_uncertain_publication_remains_charged_and_is_reported_even_when_its_anchor_is_retained() {
    let (case, tick) = published(true); let output = case.root.0.join("pending.anchor");
    let p = profile(&case, Some(&output)); let mut out = Vec::new();
    let error = run(&case.config, &p, &case.document, || ElapsedTick(tick), &mut out).unwrap_err();
    assert!(error.contains("unresolved prior deliveries")); assert!(error.contains("charges remain"));
    assert!(error.contains("no effect was resent")); assert_eq!(state(&case).executions, 0);
    assert_eq!(state(&case).control.ledger.charged, 4);
    assert_eq!(load(&output).revision(), state(&case).revision);
    let response = decode_response(out.strip_suffix(b"\n").unwrap()).unwrap();
    assert!(!matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let next = case.root.0.join("pending-again.anchor"); let p = profile(&case, Some(&next));
    out.clear();
    let error = run(&case.config, &p, &case.document, || ElapsedTick(tick + 1), &mut out).unwrap_err();
    assert!(error.contains("unresolved prior deliveries"));
    assert_eq!(state(&case).executions, 0); assert_eq!(state(&case).control.ledger.charged, 4);
    assert_eq!(load(&next).revision(), state(&case).revision);
}

#[test]
fn actual_resume_command_uses_the_guarded_profile_without_reading_live_evidence_or_qualification() {
    let (case, _) = published(false); let config_path = write_config(&case);
    let document_path = case.root.0.join("submit.json"); fs::write(&document_path, &case.document).unwrap();
    let profile_path = case.root.0.join("resume.json"); let output = case.root.0.join("cli.anchor");
    fs::write(&profile_path, profile_text(&case, Some(&output))).unwrap();
    let missing = case.root.0.join("missing-qualification");
    crate::command(vec!["resume".into(), config_path.to_str().unwrap().into(), document_path.to_str().unwrap().into(),
        "--guarded".into(), profile_path.to_str().unwrap().into(), "--credibility-activation".into(), missing.to_str().unwrap().into()]).unwrap();
    assert_eq!(state(&case).executions, 1); assert_eq!(state(&case).revision, case.before.revision + 3);
    assert_eq!(load(&output).revision(), state(&case).revision); assert!(!missing.exists());
    assert!(!case.root.0.join("evidence.json").exists());
    assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    for args in [vec!["resume", "absent", "absent", "--guarded"],
        vec!["create", "absent", "absent", "--guarded", "absent"],
        vec!["resume-checked", "absent", "absent", "--guarded", "absent"]] {
        assert!(crate::command(args.into_iter().map(String::from).collect()).unwrap_err().contains("usage:"));
    }
}
