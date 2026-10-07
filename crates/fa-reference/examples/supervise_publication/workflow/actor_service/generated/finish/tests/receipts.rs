//! Lost finish replies resolve through native history, never another closure.
use super::*;

fn completed(root: &Root, choice: ReviewDecision) -> ReviewPacket {
    initial(root, ReviewDecision::Approve, false);
    let c = closing_config(root); let p = peers(root, &c); let human = review(p.clone(), 2, choice);
    let result = run(c, recipe(false), &p, 2, || ElapsedTick(1001)).unwrap();
    let packet = human.join().unwrap(); assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(executed(&result), choice == ReviewDecision::Approve);
    assert_eq!(result.cleanup_pending, 0); packet
}

#[test]
fn lost_finish_reply_recovers_without_source_helpers_new_consent_or_numerical_resume() {
    let root = Root::new(); let packet = completed(&root, ReviewDecision::Approve);
    let before = native(&root, false).stream; let numerical = position(&root);
    let mut c = configured(&root); c.programs.clear(); let p = peers(&root, &c);
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut r = recipe(false); r.ttl_ms = 1; // Must not rebuild/renew the old finish deadline.
    let result = resume(c, r, 2, || panic!("confirmed finish needs no clock")).unwrap();
    assert!(result.failure.is_none()); assert!(executed(&result)); assert_eq!(result.cleanup_pending, 0);
    let mut output = Vec::new(); emit(result, &mut output).unwrap();
    assert!(!output.windows(7).any(|bytes| bytes == b"payload"));
    assert!(!p.socket(2).exists()); assert!(!root.0.join("evidence.json").exists());
    let after = native(&root, false).stream;
    assert_eq!(after.confirmed, before.confirmed); assert!(after.confirmed.finished());
    assert_eq!(after.publication.executions, 2);
    assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
    assert_eq!(position(&root), numerical);
    let c = configured(&root); let (host, _) = recovery::open(&c, &recipe(false)).unwrap();
    assert_eq!(host.decoder_text_finish_request(1, 2).unwrap().deadline, packet.action().spec().deadline);
    assert!(host.decoder_inspection().unwrap().paused); drop(host);
    let mut c = configured(&root); c.programs.clear(); fs::remove_file(root.0.join("evidence.json")).unwrap();
    assert!(run(c, recipe(false), &p, 2, || ElapsedTick(1002)).is_err()); // Not a new finish.
    assert_eq!(native(&root, false).stream.publication.executions, 2);
}

#[test]
fn rejected_finish_receipt_stays_rejected_without_finishing_or_refunding_prior_messages() {
    let root = Root::new(); completed(&root, ReviewDecision::Reject);
    let before = native(&root, false).stream;
    let mut c = configured(&root); c.programs.clear(); fs::remove_file(root.0.join("evidence.json")).unwrap();
    let result = resume(c, recipe(false), 2, || panic!("rejected finish is historical")).unwrap();
    assert!(result.failure.is_none()); assert!(!executed(&result));
    assert!(matches!(result.response.result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert!(emit(result, &mut Vec::new()).is_err());
    let after = native(&root, false).stream;
    assert_eq!(after.confirmed, before.confirmed); assert_eq!(after.published, before.published);
    assert!(!after.confirmed.finished()); assert_eq!(after.publication.executions, 1);
    assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
    assert_eq!(after.publication.control.ledger.reserved, 0);
}

#[test]
fn finish_receipt_refuses_wrong_predecessor_recipe_or_nonfinish_identity_without_fallback() {
    let root = Root::new(); completed(&root, ReviewDecision::Approve);
    let before = native(&root, false).stream; let numerical = position(&root);
    for mutation in 0..6 {
        let mut c = configured(&root); c.programs.clear(); let mut r = recipe(false); let mut id = 2;
        match mutation { 0 => r.request = 99, 1 => r.generation += 1,
            2 => r.text.prompt.push(b'z'), 3 => r.text.generation.scalar_products -= 1,
            4 => id = 99, _ => id = 1 }
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        assert!(resume(c, r, id, || panic!("a mismatched finish cannot request current time")).is_err());
        assert_eq!(position(&root), numerical);
        let state = native(&root, false).stream;
        assert_eq!(state.confirmed, before.confirmed); assert_eq!(state.publication.executions, 2);
        assert_eq!(state.publication.control.ledger.charged, before.publication.control.ledger.charged);
    }
}

#[test]
fn exact_finish_retry_restores_only_its_local_ticket_even_after_deadline_expiry() {
    let root = Root::new(); completed(&root, ReviewDecision::Approve);
    let c = configured(&root); let (mut host, _) = recovery::open(&c, &recipe(false)).unwrap();
    let original = host.decoder_text_finish_request(1, 2).unwrap();
    host.observe_time(host.revision(), ElapsedTick(original.deadline.0 + 1)).unwrap();
    let before = host.inspect(); let numerical = host.decoder_inspection().unwrap();
    let (port, supervisor) = host.into_generated_text_actor_gateway().unwrap();
    let driver = FileSupervisedDriver::new(supervisor);
    let mut wrong = ActorWire::new(port.clone()); let mut wire = ActorWire::new(port);
    let poll = encode_command(&Command::Poll { request: 2 }).unwrap();
    assert!(wire.exchange(&poll).result.is_err());
    let bad = encode_command(&Command::Submit { request: 2,
        proposal: FileGeneratedTextActorPort::encode_finish(2, original.target,
            original.expected_policy_epoch, ElapsedTick(original.deadline.0 + 1)).unwrap() }).unwrap();
    assert!(wrong.exchange(&bad).result.is_err()); assert!(wrong.exchange(&poll).result.is_err());
    let exact = encode_command(&Command::Submit { request: 2,
        proposal: FileGeneratedTextActorPort::encode_finish(2, original.target,
            original.expected_policy_epoch, original.deadline).unwrap() }).unwrap();
    assert!(wire.exchange(&exact).result.is_ok());
    assert!(matches!(wire.exchange(&poll).result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(driver.supervisor().host().unwrap().decoder_inspection().unwrap(), numerical);
    assert_eq!(cleanup(driver, c.timing.cleanup_ms, c.timing.poll_ms), 0);
}

#[test]
fn failing_recovered_receipt_output_does_not_republish_or_modify_the_canonical_image() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> { Err(std::io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> std::io::Result<()> { panic!("no flush after write failure") }
    }
    let root = Root::new(); completed(&root, ReviewDecision::Approve);
    let result = resume(configured(&root), recipe(false), 2, || panic!("terminal receipt")).unwrap();
    let bytes = fs::read(root.0.join("store/delivery.bin")).unwrap();
    assert!(emit(result, &mut Broken).unwrap_err().contains("no effect retry"));
    assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), bytes);
    assert_eq!(native(&root, false).stream.publication.executions, 2);
}

#[test]
fn receipt_mode_has_no_reviewer_creation_or_implicit_finish_fallback() {
    for args in [vec!["create-generated", "config", "recipe", "--resume-finish"],
        vec!["create-generated", "config", "recipe", "--resume-finish", "0"],
        vec!["create-generated", "config", "recipe", "--resume-finish", "-1"],
        vec!["create-generated", "config", "recipe", "--resume-finish", "18446744073709551616"],
        vec!["create-generated", "config", "recipe", "peer", "--resume-finish", "2"],
        vec!["create-generated", "config", "recipe", "--resume-finish", "2", "--continue-generation"]] {
        assert_eq!(super::super::super::command(&args.into_iter().map(str::to_owned).collect::<Vec<_>>(), None).unwrap_err(), USAGE);
    }
    let args = ["create-generated", "missing-config", "recipe", "--resume-finish", "2"].map(str::to_owned);
    assert_eq!(super::super::super::command(&args, Some(Path::new("qualification"))).unwrap_err(), USAGE);
    assert_ne!(super::super::super::command(&args, None).unwrap_err(), USAGE);
    let root = Root::new(); let c = configured(&root);
    assert!(resume(c, recipe(false), 2, || panic!("missing store")).is_err());
    assert!(!root.0.join("store").exists());
}
