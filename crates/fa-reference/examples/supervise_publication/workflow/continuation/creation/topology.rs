//! Initial topology intake and native bootstrap, not an alternate effect gate.
use super::{Config, FileOversight, RecoveryReserve, debug};
use crate::config::read_regular;
use super::super::checkpoint::Output;
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanReviewer,
    guarded::{FileGuardSet, mediated::FileMediatedBootstrap},
    driver::topology::FileDriverTopology,
    mediation::source::TOPOLOGY_REFRESH_READ_BYTES};
use fa_reference::action::consequence::delivery::publication_gate::PublicationLimits;
use fa_reference::perimeter_inventory::{MAX_INVENTORY_BYTES,
    topology::{LoadedAuthorityTopology, MAX_TOPOLOGY_BYTES}};
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeMap, fs, path::{Component, Path, PathBuf}};

const PROFILE_BYTES: usize = 16 * 1024;

pub(crate) struct Selection {
    topology: PathBuf,
    inventory: PathBuf,
    output: Option<PathBuf>,
}

impl Selection {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, PROFILE_BYTES)?)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let value = strict_json::parse(bytes, Limits { max_bytes: PROFILE_BYTES,
            max_depth: 2, max_items: 12, max_string_bytes: 4096 }).map_err(debug)?;
        let Json::Object(mut fields) = value else { return Err("topology creation requires an object".into()); };
        let version = fields.remove("schema_version").ok_or("missing topology creation schema")?;
        let output = match version.as_str() {
            Some("fa.mediated-create/1") => None,
            Some("fa.mediated-create/2") => Some(path(&mut fields, "retain_checkpoint")?),
            _ => return Err("unsupported topology creation profile".into()),
        };
        let topology = path(&mut fields, "topology_path")?;
        let inventory = path(&mut fields, "inventory_path")?;
        if !fields.is_empty() || topology == inventory {
            return Err("topology creation requires distinct paths and no unknown fields".into());
        }
        Ok(Self { topology, inventory, output })
    }

    pub(crate) fn prepare(self, config: &Config,
        checkpoint: &mut impl FnMut() -> Result<(), String>) -> Result<Prepared, String>
    {
        checkpoint()?;
        let parent = fs::canonicalize(config.store.parent().ok_or("store needs a parent")?).map_err(debug)?;
        let store = parent.join(config.store.file_name().ok_or("store needs a filename")?);
        for input in [&self.topology, &self.inventory] {
            if fs::canonicalize(input).map_err(debug)?.starts_with(&store) {
                return Err("topology creation inputs require custody outside the authority store".into());
            }
        }
        let topology = read_regular(&self.topology, MAX_TOPOLOGY_BYTES)?;
        checkpoint()?;
        let inventory = read_regular(&self.inventory, MAX_INVENTORY_BYTES)?;
        checkpoint()?;
        let loaded = LoadedAuthorityTopology::from_json_bytes(&topology, &inventory).map_err(debug)?;
        if loaded.graph().spec().scope != config.profile.delivery.scope
            || loaded.graph().spec().target != config.profile.delivery.target {
            return Err("topology does not match the original publication scope and target".into());
        }
        // This command necessarily performs initial and final checks. Reject an
        // impossible allowance before storage, not after a human's approval.
        let limits = loaded.source_limits();
        if limits.checks < 2 || limits.read_bytes < 2 * TOPOLOGY_REFRESH_READ_BYTES {
            return Err("topology budget must cover initial admission and final publication".into());
        }
        checkpoint()?;
        let output = self.output.as_ref().map(|path| Output::admit(config, path, topology, inventory)).transpose()?;
        if output.is_some() { checkpoint()?; }
        Ok(Prepared { selection: self, loaded, output })
    }
}

pub(crate) struct Prepared {
    selection: Selection,
    loaded: LoadedAuthorityTopology,
    output: Option<Output>,
}

impl Prepared {
    pub(crate) fn take_output(&mut self) -> Option<Output> { self.output.take() }

    pub(crate) fn create(self, config: &Config, publication: Option<PublicationLimits>)
        -> Result<(FileOversight, FileHumanReviewer, FileDriverTopology), String>
    {
        let guards = FileGuardSet { stream: None, decoder: None, decoder_stop: None,
            source: Some(config.source_policy), identity: None, campaigns: None, credential: None };
        // The FIRST image includes every selected native guard, plain witness
        // limits and original terminal reserve. No second witness-enablement
        // write and no partially bootstrapped owner can reach admission.
        let (host, roles) = FileOversight::create_mediated_guarded_reserved(
            &config.store, config.profile.clone(), &guards, None,
            FileMediatedBootstrap { topology: self.loaded.graph().clone(),
                reserve: RecoveryReserve::terminal(), publication }, None, None).map_err(debug)?;
        let source = roles.topology_observer.bind_topology_source(&host, host.revision(),
            self.selection.topology, self.selection.inventory, self.loaded).map_err(debug)?;
        Ok((host, roles.oversight.human, FileDriverTopology::new(source)))
    }
}

fn path(fields: &mut BTreeMap<String, Json>, key: &str) -> Result<PathBuf, String> {
    let value = fields.remove(key).ok_or_else(|| format!("topology creation missing {key}"))?;
    let path = PathBuf::from(value.as_str().ok_or("topology path must be a string")?);
    if !path.is_absolute() || path.file_name().is_none()
        || path.components().any(|part| matches!(part, Component::ParentDir)) {
        return Err("topology paths must be absolute without parent traversal".into());
    }
    Ok(path)
}
