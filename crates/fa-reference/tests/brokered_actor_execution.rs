//! Actor requests reach effects only through the concrete credential broker.
#![cfg(unix)]
#[path = "support/actor_gateway.rs"] mod support;

use fa_reference::action::consequence::delivery::{EndpointOutcome, EndpointStatus, FilePublicationLimits,
    NonExecutionReason, PublicationEndpoint,
    credential_broker::{BrokerCredential, BrokerRouteBinding, CredentialRevocationRequest,
        RecoverableCredentialBroker, DISPOSABLE_FILE_PROFILE}};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, IntakeLimits, Knowledge};
use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::action::{ElapsedTick, Purpose, Scope};
use fa_reference::perimeter_inventory::LoadedPerimeterInventory;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use support::{attach, proposal, review, snapshot};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        Self(std::env::temp_dir().join(format!("fa-brokered-actor-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
    }
}
impl Drop for Temp { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
fn effect_scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn inventory() -> LoadedPerimeterInventory {
    LoadedPerimeterInventory::from_json_bytes(format!(r#"{{"version":1,"families":[{{
      "scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"publication",
      "trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],
      "credentials":[{{"credential":"publish-token","holder":"broker"}}],
      "routes":[{{"route":"adapter:disposable-file","effect":"file_write",
      "profile":{{"id":"{}","generation":1}},"trust_path":["actor","enforcement"],
      "threat":"direct_credential_or_egress","actor_credential":{{"kind":"broker_mediated"}},
      "mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["reference"]}}],
      "residual_nonclaims":["reference"]}}]}}"#, DISPOSABLE_FILE_PROFILE).as_bytes()).unwrap()
}
fn binding() -> BrokerRouteBinding { BrokerRouteBinding { family: "publication".into(), route: "adapter:disposable-file".into() } }

struct Rig {
    _root: Temp,
    port: fa_reference::action::consequence::oversight::actor::ActorPort,
    supervisor: fa_reference::action::consequence::oversight::actor::ActorSupervisor,
    perimeter: RecoverableCredentialBroker,
}
impl Rig {
    fn new() -> Self {
        let root = Temp::new();
        let target = proposal().target;
        let (mut endpoint, recovery) = PublicationEndpoint::create_file_publication(&root.0, target, b"old".to_vec(),
            200, 128, FilePublicationLimits { mutations: 256, bytes: 2 * 1_048_576 }).unwrap();
        let (port, supervisor) = attach(&mut endpoint, IntakeLimits::default());
        let perimeter = RecoverableCredentialBroker::new(inventory(), binding(), effect_scope(),
            BrokerCredential::new(b"opaque-secret".to_vec()).unwrap(), endpoint, recovery).unwrap();
        Self { _root: root, port, supervisor, perimeter }
    }
}

#[test]
fn actor_supervisor_publishes_through_broker_without_raw_endpoint_access() {
    let mut rig = Rig::new();
    let ticket = rig.port.submit(1, &proposal()).unwrap();
    rig.supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut rig.supervisor, 1, 101);
    let permit = rig.supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
    let receipt = rig.supervisor.deliver_request_brokered(1, DispatchKeys::single(&permit), Some(&inputs),
        &snapshot(), rig.perimeter.broker_mut()).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(rig.perimeter.broker().payload(), b"publish");
    assert_eq!(rig.perimeter.broker().inspect().credential_exercises, 1);
    assert_eq!(rig.supervisor.broker().inspect().ledger.charged, 16);
}

#[test]
fn lost_broker_ack_is_reconciled_without_representing_the_credential() {
    let mut rig = Rig::new(); let ticket = rig.port.submit(1, &proposal()).unwrap();
    rig.supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut rig.supervisor, 1, 101);
    let permit = rig.supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
    let message = rig.supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).unwrap();
    let receipt = rig.perimeter.broker_mut().deliver(&message).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    rig.supervisor.acknowledgment_lost(1).unwrap();
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
    let before = rig.perimeter.broker().inspect().credential_exercises;
    let outcomes = rig.supervisor.reconcile_brokered_pending(rig.perimeter.broker_mut()).unwrap();
    assert!(matches!(outcomes[&1], Ok(EndpointStatus::Resolved(_))));
    assert_eq!(rig.perimeter.broker().inspect().credential_exercises, before);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
}

#[test]
fn terminal_credential_revocation_blocks_delayed_send_but_expiry_reconciliation_refunds() {
    let mut rig = Rig::new(); let ticket = rig.port.submit(1, &proposal()).unwrap();
    rig.supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut rig.supervisor, 1, 101);
    let permit = rig.supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
    let message = rig.supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).unwrap();
    rig.supervisor.acknowledgment_lost(1).unwrap();
    rig.perimeter.broker_mut().revoke_credential(CredentialRevocationRequest { operation: 1, expected_generation: 1 }).unwrap();
    assert!(rig.perimeter.broker_mut().deliver(&message).is_err());
    rig.supervisor.broker_mut().observe_time(ElapsedTick(100)).unwrap();
    rig.perimeter.broker_mut().observe_time(ElapsedTick(100)).unwrap();
    let outcomes = rig.supervisor.reconcile_brokered_pending(rig.perimeter.broker_mut()).unwrap();
    assert!(matches!(outcomes[&1], Ok(EndpointStatus::Resolved(ref receipt))
        if matches!(receipt.outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed })));
    assert_eq!(rig.perimeter.broker().inspect().credential_exercises, 0);
    assert_eq!(rig.supervisor.broker().inspect().ledger.available, 100);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::ConfirmedNotExecuted, .. }));
}

#[test]
fn dispatcher_restart_after_endpoint_reopen_preserves_original_actor_request_and_receipt() {
    let mut rig = Rig::new(); let ticket = rig.port.submit(1, &proposal()).unwrap();
    rig.supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
    let inputs = review(&mut rig.supervisor, 1, 101);
    let permit = rig.supervisor.authorize_request(1, Some(&inputs), &snapshot()).unwrap();
    let message = rig.supervisor.dispatch_request(1, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).unwrap();
    let executed = rig.perimeter.broker_mut().deliver(&message).unwrap();
    rig.supervisor.acknowledgment_lost(1).unwrap();
    let offline = rig.perimeter.into_offline();
    rig.perimeter = offline.reopen().unwrap();
    rig.supervisor.broker_mut().observe_time(ElapsedTick(2)).unwrap();
    rig.perimeter.broker_mut().observe_time(ElapsedTick(2)).unwrap();
    rig.supervisor.restart_brokered_dispatcher(rig.perimeter.broker_mut()).unwrap();
    let outcomes = rig.supervisor.reconcile_brokered_pending(rig.perimeter.broker_mut()).unwrap();
    assert!(matches!(outcomes[&1], Ok(EndpointStatus::Resolved(ref receipt)) if *receipt == executed));
    assert_eq!(rig.perimeter.broker().inspect().endpoint_executions, 1);
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
}
