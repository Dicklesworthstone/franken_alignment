//! Real artifact intake shared only by this operator lifecycle campaign.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::{
    consistency::learned::FileLearnedPublicationSnapshot,
    decoder::learned::checkpoint::FileLearnedCheckpointInfo,
    mediation::source::TOPOLOGY_REFRESH_READ_BYTES,
};
use fa_reference::action::consequence::activation::tensor::kv::model::MAX_MODEL_KV_VALUES;
use fa_reference::action::consequence::mediation::MAX_CHECK_EDGE_VISITS;

const INVENTORY: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/authority_topology_inventory.json"));

pub(super) struct Case {
    pub config: Config,
    pub config_path: PathBuf,
    pub recipe: PathBuf,
    pub root: PathBuf,
    pub selection: PathBuf,
    pub creation: PathBuf,
}

#[derive(Clone)]
pub(super) struct Pair { pub history: PathBuf, pub source: PathBuf }

impl Case {
    pub fn new(root: &Root) -> Self {
        let config_path = super::super::functional_creation::config_file(root, "functional_restart");
        let config = Config::read_learned_generation(&config_path).unwrap();
        let recipe = super::super::continuation::write(root);
        super::super::forecast::install_restart(root, &recipe, 60_000);
        evidence(root, &config, 1, true, false);
        for name in ["initial.json", "live-1.json"] {
            fs::write(root.0.join(name), graph(1, false)).unwrap();
        }
        for name in ["initial-inventory.json", "inventory-1.json"] {
            fs::write(root.0.join(name), INVENTORY).unwrap();
        }
        let selection = root.0.join("prepare-selection.json");
        fs::write(&selection, r#"{"schema":"fa.learned-generation-checkpoint/1","checkpoint":"71","expected_position":"2","expected_sampled_draws":"0"}"#).unwrap();
        let creation = root.0.join("create-topology.json");
        fs::write(&creation, format!(r#"{{"schema_version":"fa.mediated-create/2",
            "topology_path":"{}","inventory_path":"{}","retain_checkpoint":"{}"}}"#,
            root.0.join("live-1.json").display(), root.0.join("inventory-1.json").display(),
            root.0.join("prepared.topology").display())).unwrap();
        Self { config, config_path, recipe, root: root.0.clone(), selection, creation }
    }

    pub fn pair(&self, name: &str) -> Pair {
        Pair { history: self.root.join(format!("{name}.anchor")),
            source: self.root.join(format!("{name}.topology")) }
    }

    pub fn preparation_args(&self) -> Vec<String> {
        vec!["create-learned-generated".into(), self.config_path.display().to_string(),
            self.recipe.display().to_string(), "--prepare-stream-checkpoint".into(),
            self.selection.display().to_string(), "--retain-history-anchor".into(),
            self.pair("prepared").history.display().to_string(), "--topology-profile".into(),
            self.creation.display().to_string()]
    }

    pub fn prepare(&self) -> Json {
        let mut output = Vec::new();
        preparation::command(&self.preparation_args(), None, || ElapsedTick(1000), &mut output).unwrap();
        let result = report(&output);
        assert_eq!(text(&result, "outcome"), "prepared");
        assert_eq!(number(&result, "sampling_attempts"), 0);
        assert_eq!(number(&result, "executions"), 0);
        assert_eq!(result.get("topology_checkpoint_retained").and_then(Json::as_bool), Some(true));
        assert_eq!(checkpoint(&self.pair("prepared").source).history_anchor(),
            &custody::load(&self.pair("prepared").history).unwrap());
        result
    }

    pub fn view(&self) -> FileLearnedPublicationSnapshot {
        let loaded = recipe::load(&self.recipe, &self.config, false).unwrap();
        FileOversight::read_publication_with_owned_learned_consistency(&self.config.store,
            &self.config.profile, &loaded.generation,
            loaded.generation.required_owned_pre_output_forecast().unwrap()).unwrap()
    }

    pub fn disk(&self) -> Vec<u8> { fs::read(self.config.store.join("delivery.bin")).unwrap() }

    pub fn profile(&self, input: &Pair, output: &Pair, label: &str, next: Option<(u64, bool)>) -> PathBuf {
        let saved = checkpoint(&input.source);
        let (topology, inventory) = match next {
            Some((generation, bypass)) => {
                let topology = self.root.join(format!("live-{generation}.json"));
                let inventory = self.root.join(format!("inventory-{generation}.json"));
                fs::write(&topology, graph(generation, bypass)).unwrap();
                fs::write(&inventory, INVENTORY).unwrap();
                (topology, inventory)
            }
            None => (self.root.join("missing-next.json"), self.root.join("missing-next-inventory.json")),
        };
        let profile = self.root.join(format!("{label}-profile.json"));
        fs::write(&profile, format!(r#"{{"schema_version":"fa.mediated-submit/2",
            "initial":{{"topology_path":"{}","inventory_path":"{}"}},
            "next":{{"topology_path":"{}","inventory_path":"{}"}},"operation":{},
            "source_checkpoint":"{}","retain_checkpoint":"{}"}}"#,
            self.root.join("initial.json").display(), self.root.join("initial-inventory.json").display(),
            topology.display(), inventory.display(), saved.last_operation() + 1,
            input.source.display(), output.source.display())).unwrap();
        profile
    }

    pub fn control_args(&self, flag: &str, request: &Path, input: &Pair, output: &Pair,
        label: &str) -> Vec<String> {
        let profile = self.profile(input, output, label, None);
        vec!["create-learned-generated".into(), self.config_path.display().to_string(),
            self.recipe.display().to_string(), flag.into(), request.display().to_string(),
            "--history-anchor".into(), input.history.display().to_string(),
            "--retain-history-anchor".into(), output.history.display().to_string(),
            "--topology-profile".into(), profile.display().to_string()]
    }

    pub fn narrow(&self, info: FileLearnedCheckpointInfo) -> PathBuf {
        let state = self.view().publication;
        let saved = format!(r#"{{"checkpoint":"{}","actor_revision":"{}","authority_epoch":"{}",
            "control_sequence":"{}","stream":"{}","position":"{}","sampled_draws":"{}"}}"#,
            info.checkpoint, info.actor_revision, info.authority_epoch, info.control_sequence,
            info.stream, info.position, info.sampled_draws);
        let control = control_json(&state, info.actor_revision, 801, 7);
        let path = self.root.join("narrow.json");
        fs::write(&path, format!(r#"{{"schema":"fa.learned-stream-narrow/1",
            "expected_journal_revision":"{}","checkpoint":{saved},"control":{control}}}"#,
            state.revision)).unwrap();
        path
    }

    pub fn reset(&self, info: FileLearnedCheckpointInfo, first: u64) -> PathBuf {
        let control = control_json(&self.view().publication, info.actor_revision, 900, 8);
        let path = self.root.join("reset.json");
        fs::write(&path, format!(r#"{{"schema":"fa.learned-stream-reset-recovery/1",
            "checkpoint":"{}","first_incident_sequence":"{first}","control":{control},
            "restart_budget":{{"cache_values":"{MAX_MODEL_KV_VALUES}","audit":{{
            "preparation":{{"compression":{{"source_values":"1000","encoded_bytes":"65536","work_units":"1000000"}},
            "source_check":{{"source_values":"1000","encoded_bytes":"65536","reconstruction_products":"1000000"}}}},
            "monitoring":{{"encoded_bytes":"65536","probe_coordinates":"10000","reconstruction_products":"1000000",
            "materialized_values":"256","refinements":"64"}}}}}}}}"#, info.checkpoint)).unwrap();
        path
    }
}

fn control_json(state: &fa_reference::action::consequence::delivery::persistent::FileDeliverySnapshot,
    actor: u64, operation: u64, evidence: u8) -> String {
    let targets = (0..3).map(|offset| {
        let t = state.target;
        format!(r#"{{"adapter":"{}","object":"{}","contract_version":"{}","expected_version":"{}","generation":"{}"}}"#,
            t.adapter, t.object, t.contract_version, t.expected_version + offset, t.generation)
    }).collect::<Vec<_>>().join(",");
    format!(r#"{{"operation":"{operation}","expected_control_sequence":"{}","expected_actor_revision":"{actor}",
        "expected_authority_epoch":"{}","binding":{{"round":"{operation}","reducer_generation":"1",
        "evidence_root_hex":"{}"}},"retained_targets":[{targets}]}}"#,
        state.control.sequence, state.control.ledger.epoch, format!("{evidence:02x}").repeat(32))
}

pub(super) fn info(report: &Json) -> FileLearnedCheckpointInfo {
    FileLearnedCheckpointInfo { checkpoint: number(report, "checkpoint"),
        actor_revision: number(report, "actor_revision"), authority_epoch: number(report, "authority_epoch"),
        control_sequence: number(report, "control_sequence"), stream: number(report, "stream"),
        position: number(report, "position"), sampled_draws: number(report, "sampled_draws") }
}

pub(super) fn graph(generation: u64, bypass: bool) -> String {
    let extra = if bypass { r#",{"id":3,"from":1,"to":4,"channel":"dispatch","route":"publication","provenance":103}"# } else { "" };
    let reads = 32 * TOPOLOGY_REFRESH_READ_BYTES;
    format!(r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{generation},
        "complete_inventory_generation":{generation},
        "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
        "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
        "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
        "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
        "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
            {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}{extra}],
        "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
        "gates":[2],"check_edge_budget":{MAX_CHECK_EDGE_VISITS},"source_limits":{{"checks":32,"read_bytes":{reads}}}}}"#)
}
