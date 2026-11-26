//! Two resident Messages and a recovered Finish preserve independent evaluation
//! and topology work across the same publication history.
use super::super::super::mediated_topology as topology;
use super::super::anchored_recovery::inputs;
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::mediation::source::TOPOLOGY_REFRESH_READ_BYTES;

fn select(args: &mut Vec<String>, accounting: &Path, peer: &Path, source: &Path) {
    args.extend([
        "--andon".into(),
        accounting.display().to_string(),
        "--evaluator-profile".into(),
        peer.display().to_string(),
        "--topology-profile".into(),
        source.display().to_string(),
    ]);
}

fn run(
    root: &Root,
    args: &[String],
    peers: &mut Peers,
    decisions: [Option<ReviewDecision>; 3],
    mut child: EvaluatorChild,
) {
    let result = resident::command_with_clock(args, None, || {
        poll(peers, root, decisions);
        crate::workflow::clock()
    });
    fs::write(root.0.join("service.finished"), b"resident service ended").unwrap();
    assert!(result.is_ok(), "{result:?}");
    child.finish();
    fs::remove_file(root.0.join("service.finished")).unwrap();
}

#[test]
fn two_mediated_messages_and_anchored_finish_retain_reviews_source_work_and_late_labels() {
    let root = Root::new();
    let (config, paths, entries) = selected(&root, None, None);
    let reader = Reader::new(&root, &paths[0]);
    let source = topology::creation_profile(&root, false);
    let accounting = joint_accounting_file(&root);
    let child = EvaluatorChild::start(&root, "messages");
    let peer = peer_file(&root, &entries[0].actor, child.0.id());
    let mut args = inputs(&root, &config, &paths[..2], &entries[..2]);
    let first_history = root.0.join("messages.anchor");
    let output = args.iter().position(|arg| arg == "--retain-history-anchor").unwrap() + 1;
    args[output] = first_history.display().to_string();
    select(&mut args, &accounting, &peer, &source);

    let approve = Some(ReviewDecision::Approve);
    let mut peers = Peers::new(&root, &config, &entries);
    run(&root, &args, &mut peers, [approve, approve, None], child);
    assert_eq!(peers.seen_human, [true, true, false]);
    assert_eq!(peers.seen_actor, [true, true, false]);
    let (responses, packets) = peers.join();
    assert!(responses[..2].iter().all(|r| confirmed(r.as_ref().unwrap())));
    let first = packets[0].as_ref().unwrap();
    let second = packets[1].as_ref().unwrap();
    assert_ne!(first.binding().session, second.binding().session);
    let epoch = first.action().spec().policy_epoch;
    assert_eq!(epoch, second.action().spec().policy_epoch);
    let published = reader.snapshot();
    assert_eq!(published.executions, 2);
    assert_eq!(published.payload, b"AA");
    assert_eq!(
        published.control.ledger.charged,
        first.action().spec().units + second.action().spec().units
    );
    let previous = topology::checkpoint(&root, 1);
    assert!(previous.available());
    assert_eq!(previous.history_anchor(), &archive::load(&first_history).unwrap());
    assert_eq!(published.revision, previous.history_anchor().revision());
    let labels = read_json(&root.0.join("receipt-messages.json"));
    assert!(previous.history_anchor().revision() >= number(&labels, &["after", "journal_revision"]));
    assert!(previous.work().checks >= 4);
    let saved_history = fs::read(&first_history).unwrap();
    let saved_source = fs::read(root.0.join("source-1.bin")).unwrap();

    evidence(&root, &config, 2, true, false);
    let continuation = topology::continuation_profile(&root, &previous, 1);
    let minimum = number(&labels, &["after", "evaluation_revision"]);
    let current = fs::read_to_string(&accounting).unwrap();
    let exact = current.replace(
        "\"minimum_evaluation_revision\": 0",
        &format!("\"minimum_evaluation_revision\": {minimum}"),
    );
    assert_ne!(exact, current);
    let child = EvaluatorChild::start(&root, "final");
    let peer = peer_file(&root, &entries[2].actor, child.0.id());
    let mut args = inputs(&root, &config, &paths[2..], &entries[2..]);
    args.extend([
        "--after-message".into(),
        "2".into(),
        "--history-anchor".into(),
        first_history.display().to_string(),
    ]);
    select(&mut args, &accounting, &peer, &continuation);

    // Changing only the independent joint policy must refuse before the Fence.
    let changed = exact.replace("\"id\": 71", "\"id\": 72");
    assert_ne!(changed, exact);
    fs::write(&accounting, changed).unwrap();
    let canonical = config.store.join("delivery.bin");
    let bytes = fs::read(&canonical).unwrap();
    assert!(resident::command_with_clock(&args, None, crate::workflow::clock).is_err());
    assert_eq!(fs::read(&canonical).unwrap(), bytes);
    assert!(!root.0.join("source-2.bin").exists());
    assert!(!root.0.join("final.anchor").exists());
    quiescent(&root);
    fs::write(&accounting, exact).unwrap();

    let mut peers = Peers::new(&root, &config, &entries);
    run(&root, &args, &mut peers, [None, None, approve], child);
    assert_eq!(peers.seen_human, [false, false, true]);
    assert_eq!(peers.seen_actor, [false, false, true]);
    let (responses, packets) = peers.join();
    assert!(confirmed(responses[2].as_ref().unwrap()));
    let packet = packets[2].as_ref().unwrap();
    let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
    assert!(frame.is_finish());
    assert_eq!(frame.prior_messages(), &["A", "A"]);
    assert!(packet.action().spec().policy_epoch > epoch);
    let finished = reader.snapshot();
    assert_eq!(finished.executions, 3);
    assert_eq!(finished.payload, b"AA");
    assert_eq!(
        finished.control.ledger.charged,
        published.control.ledger.charged + packet.action().spec().units
    );
    assert_eq!(finished.control.ledger.reserved, 0);
    let saved = topology::checkpoint(&root, 2);
    assert_eq!(saved.history_anchor(), &archive::load(&root.0.join("final.anchor")).unwrap());
    assert_eq!(saved.history_anchor().revision(), finished.revision);
    assert!(saved.available());
    assert!(saved.work().checks > previous.work().checks);
    assert_eq!(saved.work().reserved_read_bytes, saved.work().checks * TOPOLOGY_REFRESH_READ_BYTES);
    assert!(saved.last_operation() > previous.last_operation());
    assert_eq!(saved.expected().graph().spec().generation, 2);
    let final_label = read_json(&root.0.join("receipt-final.json"));
    assert!(number(&final_label, &["after", "evaluation_revision"]) > minimum);
    assert!(saved.history_anchor().revision() >= number(&final_label, &["after", "journal_revision"]));
    assert_eq!(fs::read(&first_history).unwrap(), saved_history);
    assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), saved_source);
    quiescent(&root);
}

#[test]
fn future_actor_socket_cannot_take_the_topology_checkpoint_destination() {
    let root = Root::new();
    let (config, paths, entries) = selected(&root, None, None);
    let source = topology::creation_profile(&root, false);
    let document = fs::read_to_string(&source).unwrap();
    let collision = document.replace(
        root.0.join("source-1.bin").to_str().unwrap(),
        entries[1].actor.socket.to_str().unwrap(),
    );
    assert_ne!(document, collision);
    fs::write(&source, collision).unwrap();
    let accounting = accounting_file(&root);
    let peer = peer_file(&root, &entries[0].actor, std::process::id().checked_add(1).unwrap());
    let mut args = inputs(&root, &config, &paths[..2], &entries[..2]);
    select(&mut args, &accounting, &peer, &source);
    assert!(resident::command_with_clock(&args, None, || panic!("collision acquired clock")).is_err());
    assert!(!config.store.exists());
    assert!(!root.0.join("final.anchor").exists());
    assert!(!entries[1].actor.socket.exists());
    assert!(!evaluator::Settings::read(&peer).unwrap().socket_path().exists());
}
