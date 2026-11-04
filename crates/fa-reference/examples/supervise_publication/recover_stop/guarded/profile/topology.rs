//! Explicit independent topology requirements for terminal operator recovery.
//! Reuse the original bounded topology/inventory decoder, never stored defaults.
use super::{Config, Json, Path, PathBuf, absolute, debug, end, fs, object, read_regular, take};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::FileTopologyRequirement;
use fa_reference::action::consequence::mediation::AuthorityGraph;
use fa_reference::perimeter_inventory::{MAX_INVENTORY_BYTES,
    topology::{LoadedAuthorityTopology, MAX_TOPOLOGY_BYTES}};

pub(super) struct Selection {
    initial_topology: PathBuf,
    initial_inventory: PathBuf,
    current_topology: PathBuf,
    current_inventory: PathBuf,
    available: bool,
}

impl Selection {
    pub(super) fn decode(value: Json) -> Result<Self, String> {
        let mut fields = object(value)?;
        let result = Self {
            initial_topology: absolute(take(&mut fields, "initial_topology")?)?,
            initial_inventory: absolute(take(&mut fields, "initial_inventory")?)?,
            current_topology: absolute(take(&mut fields, "current_topology")?)?,
            current_inventory: absolute(take(&mut fields, "current_inventory")?)?,
            available: take(&mut fields, "available")?.as_bool()
                .ok_or("topology availability must be an explicit boolean")?,
        };
        end(&fields)?;
        Ok(result)
    }

    pub(super) fn admit(&self, config: &Config,
        checkpoint: &mut impl FnMut() -> Result<(), String>)
        -> Result<FileTopologyRequirement, String>
    {
        checkpoint()?;
        let store = fs::canonicalize(&config.store).map_err(debug)?;
        let initial = load(&self.initial_topology, &self.initial_inventory, &store, checkpoint)?;
        let current = load(&self.current_topology, &self.current_inventory, &store, checkpoint)?;
        // No "check"/certify call: these are the operator's required historical
        // graphs. The native mediated cursor compares every original guard and
        // transition before it can return stopped observations, never a permit.
        Ok(FileTopologyRequirement { initial, current, available: self.available })
    }
}

fn independent(path: &Path, store: &Path, maximum: usize,
    checkpoint: &mut impl FnMut() -> Result<(), String>) -> Result<Vec<u8>, String>
{
    checkpoint()?;
    let canonical = fs::canonicalize(path).map_err(debug)?;
    if canonical.starts_with(store) {
        return Err("topology recovery inputs require custody outside the authority store".into());
    }
    // Read the originally selected path so read_regular still refuses a final
    // symlink. The same trusted-parent assumptions as anchor custody apply.
    let bytes = read_regular(path, maximum)?;
    checkpoint()?;
    Ok(bytes)
}

fn load(topology: &Path, inventory: &Path, store: &Path,
    checkpoint: &mut impl FnMut() -> Result<(), String>) -> Result<AuthorityGraph, String>
{
    let topology = independent(topology, store, MAX_TOPOLOGY_BYTES, checkpoint)?;
    let inventory = independent(inventory, store, MAX_INVENTORY_BYTES, checkpoint)?;
    let loaded = LoadedAuthorityTopology::from_json_bytes(&topology, &inventory).map_err(debug)?;
    checkpoint()?;
    Ok(loaded.graph().clone())
}
