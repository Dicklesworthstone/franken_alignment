//! One original credential capability for the actual learned publication owner.
//! Historical selection contains only independently supplied nonsecret contracts.
use super::{Config, Deadline, ElapsedTick, FileOversight, Loaded, checked, debug,
    fields::{Assets, Fields, MAX_DATA_BYTES}, identity, recipe::joint};
use fa_reference::action::consequence::delivery::{
    credential_broker::{BrokerCredential, ProviderCredential, BrokerRouteBinding, MAX_BROKER_CREDENTIAL_BYTES},
    persistent::observed::{credential::{FileCredentialPermit, FileCredentialPolicy},
        guarded::{FileCredentialEpoch, FileCredentialRegistration}},
};
use fa_reference::perimeter_inventory::{LoadedPerimeterInventory, MAX_INVENTORY_BYTES};
use fa_reference::strict_json::{self, Json, Limits};
use std::{fs, os::unix::fs::MetadataExt, path::{Path, PathBuf}};

const SCHEMA: &str = "fa.learned-credential-publication/1";
const PROFILE_SCHEMA: &str = "fa.learned-file-credential/1";
const MAX_RECIPE_BYTES: usize = 64 * 1024;
const MAX_PROFILE_BYTES: usize = 64 * 1024;

type Decoded = (Fields, Option<joint::HeldOutJointPolicy>, Option<identity::profile::Profile>,
    Option<PathBuf>, Option<PathBuf>);

/// Canonical composition: credential(checked(identity(joint(legacy)))). Each
/// existing inner envelope remains optional; duplicate/reordered wrappers refuse.
pub(super) fn decode(bytes: &[u8]) -> Result<Decoded, String> {
    let json = strict_json::parse(bytes, Limits { max_bytes: MAX_RECIPE_BYTES, max_depth: 14,
        max_items: 262_144, max_string_bytes: MAX_DATA_BYTES }).map_err(debug)?;
    if json.get("schema").and_then(Json::as_str) != Some(SCHEMA) {
        let (recipe, joint, identity, publication) = checked::decode(bytes)?;
        return Ok((recipe, joint, identity, publication, None));
    }
    let mut wrapper = Fields::new(json)?;
    wrapper.text("schema")?;
    let credential = wrapper.path("credential")?;
    let recipe = wrapper.take("recipe")?;
    wrapper.end()?;
    let (recipe, joint, identity, publication) = checked::unwrap(recipe)?;
    Ok((recipe, joint, identity, publication, Some(credential)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Selection {
    pub policy: FileCredentialPolicy,
    pub epoch: FileCredentialEpoch,
    inventory_path: PathBuf,
    broker_path: PathBuf,
    provider_path: PathBuf,
    binding: BrokerRouteBinding,
    inventory: Option<LoadedPerimeterInventory>,
    remaining_assets: usize,
}

impl Selection {
    pub(super) fn load(path: &Path, config: &Config, assets: &mut Assets, live: bool)
        -> Result<Self, String>
    {
        let mut profile = Fields::parse(&assets.read(path, MAX_PROFILE_BYTES)?, MAX_PROFILE_BYTES)?;
        if profile.text("schema")? != PROFILE_SCHEMA { return Err("unsupported learned credential profile".into()); }
        let mut policy = profile.object("policy")?;
        let policy_value = FileCredentialPolicy { family: policy.text("family")?, route: policy.text("route")?,
            credential: policy.text("credential")?, profile_generation: policy.number("profile_generation")? };
        policy.end()?;
        let mut epoch = profile.object("epoch")?;
        let generation = epoch.number("generation")?;
        let revoked = epoch.take("revoked")?.as_bool().ok_or("credential revoked must be a boolean")?;
        epoch.end()?;
        if generation == 0 { return Err("credential generation must be nonzero".into()); }
        let inventory_path = profile.path("inventory")?;
        let broker_path = profile.path("broker_credential")?;
        let provider_path = profile.path("provider_credential")?;
        profile.end()?;
        if broker_path == provider_path { return Err("broker and provider credentials require separate input files".into()); }
        let inventory = if live {
            let inventory = LoadedPerimeterInventory::from_json_bytes(
                &assets.read(&inventory_path, MAX_INVENTORY_BYTES)?).map_err(debug)?;
            let actual = FileCredentialPolicy::from_inventory(&inventory,
                &BrokerRouteBinding { family: policy_value.family.clone(), route: policy_value.route.clone() },
                config.profile.delivery.scope, config.profile.delivery.target).map_err(debug)?;
            if actual != policy_value { return Err("credential profile differs from the original resolved route".into()); }
            Some(inventory)
        } else { None };
        let binding = BrokerRouteBinding { family: policy_value.family.clone(), route: policy_value.route.clone() };
        Ok(Self { policy: policy_value, epoch: FileCredentialEpoch { generation, revoked },
            inventory_path, broker_path, provider_path, binding, inventory, remaining_assets: 0 })
    }

    pub(super) fn retain_allowance(&mut self, assets: &Assets) { self.remaining_assets = assets.remaining(); }

    pub(super) fn same_resident_selection(&self, other: &Self) -> bool {
        self.policy == other.policy && self.epoch == other.epoch
            && self.inventory_path == other.inventory_path && self.inventory == other.inventory
            && self.broker_path == other.broker_path && self.provider_path == other.provider_path
    }
}

pub(super) fn check_selection(loaded: &Loaded) -> Result<(), String> {
    if loaded.generation.required_credential_policy() != loaded.credential.as_ref().map(|c| &c.policy) {
        return Err("learned credential profile differs from the exact numerical recipe".into());
    }
    Ok(())
}

pub(super) fn check_creation(loaded: &Loaded) -> Result<(), String> {
    check_selection(loaded)?;
    if loaded.credential.as_ref().is_some_and(|c| c.epoch != (FileCredentialEpoch { generation: 1, revoked: false })) {
        return Err("new learned credential owner requires original active generation one".into());
    }
    Ok(())
}

/// This is registration data, never a credential permit. Native bootstrap
/// independently resolves it again before publishing the first canonical image.
pub(super) fn registration(loaded: &Loaded) -> Result<Option<FileCredentialRegistration<'_>>, String> {
    loaded.credential.as_ref().map(|selected| Ok(FileCredentialRegistration {
        inventory: selected.inventory.as_ref().ok_or("live credential inventory is unavailable")?,
        binding: &selected.binding,
    })).transpose()
}

struct PreparedCredentials {
    inventory: LoadedPerimeterInventory,
    binding: BrokerRouteBinding,
    broker: BrokerCredential,
    provider: ProviderCredential,
}

/// One live run owns this capability, including every resident successor. There
/// is no clone, secret accessor or rebind on a catch-up poll or next request.
pub(super) struct Session {
    selected: Option<(FileCredentialPolicy, FileCredentialEpoch)>,
    prepared: Option<PreparedCredentials>,
    permit: Option<FileCredentialPermit>,
}

/// Preflight both independent inputs BEFORE sockets, authority creation or
/// recovery. Failure therefore leaves create/recovery retryable. Exact raw bytes
/// are bounded and charged once; no trimming or derivation of the other input.
pub(super) fn prepare<F>(loaded: &Loaded, continuing: bool, deadline: &Deadline, time: &mut F)
    -> Result<Session, String>
where F: FnMut() -> ElapsedTick {
    check_selection(loaded)?;
    if !continuing { check_creation(loaded)?; }
    let Some(selected) = &loaded.credential else {
        return Ok(Session { selected: None, prepared: None, permit: None });
    };
    if selected.epoch.revoked { return Err("selected credential generation is revoked; live execution is unavailable".into()); }
    let inventory = selected.inventory.as_ref().ok_or("live credential inventory is unavailable")?;
    deadline.check(time())?;
    let left = fs::metadata(&selected.broker_path).map_err(|_| "broker credential input is unavailable")?;
    let right = fs::metadata(&selected.provider_path).map_err(|_| "provider credential input is unavailable")?;
    if left.dev() == right.dev() && left.ino() == right.ino() {
        return Err("broker and provider credentials require separate input files".into());
    }
    let mut assets = Assets::new(selected.remaining_assets)?;
    let broker = assets.read(&selected.broker_path, MAX_BROKER_CREDENTIAL_BYTES)
        .map_err(|_| "broker credential input is unavailable or exceeds its bounded allowance")?;
    deadline.check(time())?;
    let provider = assets.read(&selected.provider_path, MAX_BROKER_CREDENTIAL_BYTES)
        .map_err(|_| "provider credential input is unavailable or exceeds its bounded allowance")?;
    deadline.check(time())?;
    if broker != provider { return Err("independently supplied broker and provider credentials do not match".into()); }
    let prepared = PreparedCredentials { inventory: inventory.clone(), binding: selected.binding.clone(),
        broker: BrokerCredential::new(broker).map_err(debug)?, provider: ProviderCredential::new(provider).map_err(debug)? };
    Ok(Session { selected: Some((selected.policy.clone(), selected.epoch)), prepared: Some(prepared), permit: None })
}

impl Session {
    pub(super) fn bind(&mut self, loaded: &Loaded, host: &FileOversight) -> Result<(), String> {
        check_selection(loaded)?;
        let expected = loaded.credential.as_ref().map(|c| (c.policy.clone(), c.epoch));
        if self.selected != expected { return Err("resident credential selection changed".into()); }
        let actual = host.credential_status().map_err(debug)?.map(|status|
            (status.policy, FileCredentialEpoch { generation: status.generation, revoked: status.revoked }));
        if actual != expected { return Err("live credential policy or epoch differs from the independent selection".into()); }
        if let Some(prepared) = self.prepared.take() {
            self.permit = Some(host.bind_credential_pair(host.revision(), &prepared.inventory, &prepared.binding,
                prepared.broker, prepared.provider).map_err(debug)?);
        }
        if self.selected.is_some() != self.permit.is_some() {
            return Err("original live credential capability is unavailable".into());
        }
        Ok(())
    }

    pub(super) fn permit(&self) -> Option<&FileCredentialPermit> { self.permit.as_ref() }
}
