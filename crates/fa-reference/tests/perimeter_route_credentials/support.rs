//! Original actor/congress authority and actual independently owned file sinks.
use crate::gateway;
use fa_reference::action::consequence::delivery::{
    FileEndpointRecovery, FilePublicationLimits, PublicationEndpoint,
    credential_broker::{
        BrokerCredential, BrokerRouteBinding, DISPOSABLE_FILE_PROFILE, ProviderCredential,
        RecoverableCredentialBroker,
    },
};
use fa_reference::action::consequence::oversight::{
    CommitteeInput,
    actor::{ActorPort, ActorSupervisor, ActorTicket, IntakeLimits},
};
use fa_reference::action::{Permit, Purpose, Scope};
use fa_reference::perimeter::PerimeterScope;
use fa_reference::perimeter_inventory::LoadedPerimeterInventory;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const ROUTES: [&str; 2] = ["adapter:provider-a", "adapter:provider-b"];
pub const NAMES: [&str; 2] = ["provider-a-token", "provider-b-token"];
pub const SECRETS: [&[u8]; 2] = [
    b"independently-provisioned-route-a-credential-3842",
    b"independently-provisioned-route-b-credential-6197",
];
pub const ROTATED: &[u8] = b"independently-provisioned-route-a-replacement-7501";

pub fn scope() -> Scope {
    Scope {
        tenant: 1,
        principal: 2,
        run: 3,
        branch: 4,
        authority: 5,
        purpose: Purpose::Effect,
    }
}

pub fn perimeter_scope() -> PerimeterScope {
    PerimeterScope {
        tenant: 1,
        principal: 2,
        purpose: 1,
    }
}

pub fn binding(index: usize) -> BrokerRouteBinding {
    BrokerRouteBinding {
        family: "publication".into(),
        route: ROUTES[index].into(),
    }
}

pub fn credential_json(name: &str, holder: &str) -> String {
    format!(r#"{{"credential":"{name}","holder":"{holder}"}}"#)
}

pub fn route_json(route: &str, profile: &str, selection: Option<&str>) -> String {
    let credential = selection.map_or(String::new(), |name| format!(r#", "credential":"{name}""#));
    format!(
        r#"{{"route":"{route}","effect":"file_write","profile":{{"id":"{profile}","generation":1}},"trust_path":["actor","enforcement"],"threat":"direct_credential_or_egress","actor_credential":{{"kind":"broker_mediated"{credential}}},"mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["route-residual"]}}"#
    )
}

pub fn family_json(family: &str, credentials: &[String], routes: &[String]) -> String {
    let credentials = credentials.join(",");
    let routes = routes.join(",");
    format!(
        r#"{{"scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"{family}","trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],"credentials":[{credentials}],"routes":[{routes}],"residual_nonclaims":["operator filesystem and provisioning are trusted"]}}"#
    )
}

pub fn document(version: u64, families: &[String]) -> String {
    format!(
        r#"{{"version":{version},"families":[{}]}}"#,
        families.join(",")
    )
}

pub fn inventory_json(version: u64, credentials: usize) -> String {
    let declared = NAMES[..credentials]
        .iter()
        .map(|name| credential_json(name, "broker"))
        .collect::<Vec<_>>();
    let routes = ROUTES
        .iter()
        .enumerate()
        .map(|(index, route)| {
            route_json(
                route,
                DISPOSABLE_FILE_PROFILE,
                (version == 2).then_some(NAMES[index]),
            )
        })
        .collect::<Vec<_>>();
    document(version, &[family_json("publication", &declared, &routes)])
}

pub fn inventory() -> LoadedPerimeterInventory {
    LoadedPerimeterInventory::from_json_bytes(inventory_json(2, 2).as_bytes()).unwrap()
}

static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct Directory(PathBuf);

impl Directory {
    pub fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fa-route-credentials-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
    pub fn endpoint(&self) -> PathBuf {
        self.0.join("endpoint")
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("route credential fixture cleanup: {error}");
        }
    }
}

pub struct Unbound {
    pub root: Directory,
    pub port: ActorPort,
    pub supervisor: ActorSupervisor,
    pub endpoint: Option<PublicationEndpoint>,
    pub recovery: Option<FileEndpointRecovery>,
}

impl Unbound {
    pub fn new() -> Self {
        let root = Directory::new();
        // Both independently owned endpoints intentionally have equal numeric
        // target fields. Only the original process-local binding distinguishes
        // their effect authority; this is stronger than an object-ID mismatch.
        let (mut endpoint, recovery) = PublicationEndpoint::create_file_publication(
            root.endpoint(),
            gateway::proposal().target,
            b"old".to_vec(),
            200,
            128,
            FilePublicationLimits {
                mutations: 256,
                bytes: 2 * 1_048_576,
            },
        )
        .unwrap();
        let (port, supervisor) = gateway::attach(&mut endpoint, IntakeLimits::default());
        Self {
            root,
            port,
            supervisor,
            endpoint: Some(endpoint),
            recovery: Some(recovery),
        }
    }

    pub fn bind(mut self, index: usize) -> Rig {
        // The independently specified fixture inputs construct separate opaque
        // capability types. Neither side is obtained by extracting the other.
        let perimeter = RecoverableCredentialBroker::new(
            inventory(),
            binding(index),
            scope(),
            BrokerCredential::new(SECRETS[index].to_vec()).unwrap(),
            ProviderCredential::new(SECRETS[index].to_vec()).unwrap(),
            self.endpoint.take().unwrap(),
            self.recovery.take().unwrap(),
        )
        .unwrap();
        Rig {
            root: self.root,
            port: self.port,
            supervisor: self.supervisor,
            perimeter,
        }
    }
}

pub struct Rig {
    pub root: Directory,
    pub port: ActorPort,
    pub supervisor: ActorSupervisor,
    pub perimeter: RecoverableCredentialBroker,
}

impl Rig {
    pub fn new(index: usize) -> Self {
        Unbound::new().bind(index)
    }

    pub fn reviewed(
        &mut self,
        request: u64,
        payload: &[u8],
    ) -> (ActorTicket, CommitteeInput, Permit) {
        let mut proposal = gateway::proposal();
        proposal.payload = payload.to_vec();
        let ticket = self.port.submit(request, &proposal).unwrap();
        self.supervisor
            .accept_next(&gateway::snapshot())
            .unwrap()
            .unwrap()
            .result
            .unwrap();
        let inputs = gateway::review(&mut self.supervisor, request, request + 100);
        let permit = self
            .supervisor
            .authorize_request(request, Some(&inputs), &gateway::snapshot())
            .unwrap();
        (ticket, inputs, permit)
    }

    pub fn assert_no_secret(&self) {
        let bytes = fs::read(self.root.endpoint().join("publication.bin")).unwrap();
        for secret in SECRETS.into_iter().chain([ROTATED]) {
            assert!(!bytes.windows(secret.len()).any(|part| part == secret));
            assert!(
                !format!("{:?}", self.perimeter).contains(std::str::from_utf8(secret).unwrap())
            );
        }
    }
}
