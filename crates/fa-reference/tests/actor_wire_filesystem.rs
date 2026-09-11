//! External byte requests reach the real file endpoint without exporting authority.
#![cfg(unix)]
#[path = "support/actor_gateway.rs"]
mod support;

use fa_reference::action::consequence::oversight::actor::IntakeLimits;
use fa_reference::action::consequence::oversight::actor_wire::{ActorChannel, ActorWire, ChannelLimits, Command, encode_command};
use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::action::consequence::delivery::{PublicationEndpoint, filesystem::FilePublicationLimits};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::strict_json::{self, Json, Limits};
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use support::{attach, proposal, review, snapshot};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-wire-file-{}-{tick}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap(); Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("wire fixture cleanup: {error}"); } }
}
fn exchange(channel: &mut ActorChannel, command: Command) -> Json {
    let mut request = encode_command(&command).unwrap(); request.push(b'\n');
    assert_eq!(channel.feed(&request).consumed, request.len());
    let mut reply = Vec::new(); channel.write_once(&mut reply).unwrap();
    strict_json::parse(&reply, Limits::default()).unwrap()
}

#[test]
fn byte_gateway_lost_ack_file_reopen_and_retry_preserve_one_real_publication() {
    let root = Temp::new();
    let (mut endpoint, recovery) = PublicationEndpoint::create_file_publication(
        root.0.join("publication"), proposal().target, b"old".to_vec(), 200, 128,
        FilePublicationLimits { mutations: 128, bytes: 1_048_576 },
    ).unwrap();
    let (port, mut supervisor) = attach(&mut endpoint, IntakeLimits::default());
    let mut channel = ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap();
    let response = exchange(&mut channel, Command::Submit { request: 42, proposal: proposal() });
    assert_eq!(response.get("knowledge").unwrap().get("state").unwrap().as_str(), Some("pending"));
    assert!(supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap().is_some());
    assert!(supervisor.authorize_request(42, None, &snapshot()).is_err());
    assert_eq!(PublicationEndpoint::read_file_publication(recovery.directory()).unwrap().payload, b"old");
    let inputs = review(&mut supervisor, 42, 1);
    let permit = supervisor.authorize_request(42, Some(&inputs), &snapshot()).unwrap();
    let message = supervisor.dispatch_request(42, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&message).unwrap();
    supervisor.acknowledgment_lost(42).unwrap();
    let id = supervisor.attempt(42).unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.stages[&id], ActionState::Unknown);
    assert_eq!(supervisor.broker().inspect().ledger.charged, 16);
    let response = exchange(&mut channel, Command::Cancel { request: 42 });
    assert_eq!(response.get("knowledge").unwrap().get("state").unwrap().as_str(), Some("unknown"));
    drop(endpoint);
    let revision = supervisor.broker().input_revision(id).unwrap();
    supervisor.broker_mut().inputs_unavailable(id, revision).unwrap();
    let mut endpoint = recovery.reopen().unwrap();
    endpoint.observe_time(ElapsedTick(2)).unwrap();
    let fence = supervisor.broker_mut().restart_dispatcher().unwrap();
    supervisor.broker_mut().confirm_fence(endpoint.install_fence(fence).unwrap()).unwrap();
    let statuses = supervisor.reconcile_pending(&mut endpoint).unwrap();
    assert!(statuses[&id].is_ok());
    assert!(!supervisor.accept_receipt(receipt).unwrap());
    channel = ActorChannel::new(channel.into_wire(), ChannelLimits::default()).unwrap();
    let response = exchange(&mut channel, Command::Submit { request: 42, proposal: proposal() });
    assert_eq!(response.get("knowledge").unwrap().get("value").unwrap().as_str(), Some("executed"));
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    let actual = PublicationEndpoint::read_file_publication(recovery.directory()).unwrap();
    assert_eq!(actual.payload, b"publish"); assert_eq!(actual.execution_count, 1);
    assert_eq!(supervisor.broker().inspect().ledger.charged, 16);
    // Differential: the same wire request and congress produce the same visible
    // terminal observation through the independent memory endpoint profile.
    let (memory_port, mut memory_host, mut memory_endpoint) = support::fixture(IntakeLimits::default());
    let mut memory_channel = ActorChannel::new(ActorWire::new(memory_port), ChannelLimits::default()).unwrap();
    exchange(&mut memory_channel, Command::Submit { request: 42, proposal: proposal() });
    assert!(memory_host.accept_next(&snapshot()).unwrap().unwrap().result.unwrap().is_some());
    let memory_inputs = review(&mut memory_host, 42, 1);
    let memory_permit = memory_host.authorize_request(42, Some(&memory_inputs), &snapshot()).unwrap();
    let memory_message = memory_host.dispatch_request(42, DispatchKeys::single(&memory_permit), Some(&memory_inputs), &snapshot()).unwrap();
    let memory_receipt = memory_endpoint.deliver(&memory_message).unwrap();
    assert_eq!(supervisor.accept_receipt(memory_receipt.clone()), Err(fa_reference::Error::Binding));
    memory_host.accept_receipt(memory_receipt).unwrap();
    let memory_response = exchange(&mut memory_channel, Command::Poll { request: 42 });
    assert_eq!(response, memory_response);
    assert_eq!(actual.payload, memory_endpoint.payload());
    assert_eq!(actual.execution_count, memory_endpoint.execution_count());
}
