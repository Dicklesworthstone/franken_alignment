//! Real numerical histories and independently retained fleet registration inputs.
use super::*;
use fa_reference::action::consequence::delivery::persistent::{
    FileDeliverySnapshot,
    observed::{
        decoder::learned::FileLearnedConfig,
        guarded::anchored::FileHistoryAnchor,
        shutdown::{FileShutdownDomain, FileShutdownMember, FileShutdownPlan},
    },
};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Cursor,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

pub(super) const INPUT_LIMIT: usize = 128 * 1024 * 1024;
pub(super) const OPERATION: u64 = 900;
pub(super) const WALL: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug)]
pub(super) enum Model {
    Ordinary,
    Plain,
    Raw,
    Owned,
}

pub(super) struct Member {
    pub id: u64,
    pub root: Root,
    pub config: Config,
    pub config_path: PathBuf,
    pub recipe: Option<PathBuf>,
    pub anchor: PathBuf,
    pub before: FileDeliverySnapshot,
    pub generation: Option<FileLearnedConfig>,
}

pub(super) fn private(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
}

impl Member {
    pub fn new(id: u64, model: Model, dispatched: bool) -> Self {
        let root = Root::new();
        let config_path = root.0.join("fleet-member.json");
        let text = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/supervised_publication.json"
        ))
        .replace(
            "/operator/publication-store",
            root.0.join("store").to_str().unwrap(),
        )
        .replace(
            "/operator/evidence.json",
            root.0.join("evidence.json").to_str().unwrap(),
        )
        .replace("\"run\": 3", &format!("\"run\": {id}"))
        .replace(
            "\"initial_payload_hex\": \"696e697469616c\"",
            "\"initial_payload_hex\": \"\"",
        )
        .replace("\"events\": 256", "\"events\": 1024");
        private(&config_path, text.as_bytes());
        let mut config = Config::read(&config_path).unwrap();
        config.programs.clear();
        let anchor = root.0.join("fleet-member.anchor");
        if matches!(model, Model::Ordinary) {
            assert!(!dispatched);
            let (mut host, _) =
                FileOversight::create(&config.store, config.profile.clone()).unwrap();
            host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
                .unwrap();
            host.enable_publication_guard(host.revision()).unwrap();
            host.observe_time(host.revision(), ElapsedTick(1000))
                .unwrap();
            let before = host.inspect();
            let mut bytes = Vec::new();
            host.history_anchor()
                .unwrap()
                .write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES)
                .unwrap();
            private(&anchor, &bytes);
            drop(host);
            return Self {
                id,
                root,
                config,
                config_path,
                recipe: None,
                anchor,
                before,
                generation: None,
            };
        }
        let path = fixture::write(&root, b"allow");
        let prediction_mode = match model {
            Model::Raw => Some(false),
            Model::Owned => Some(true),
            _ => None,
        };
        if let Some(owned) = prediction_mode {
            super::super::forecast::install(&root, &path, owned, 60_000);
        }
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let (host, reviewer, observer) = if prediction_mode.is_some() {
            let (host, reviewer, observer) =
                FileOversight::create_with_pre_output_forecast_with_reserve(
                    &config.store,
                    config.profile.clone(),
                    loaded.generation.clone(),
                    RecoveryReserve::terminal(),
                )
                .unwrap();
            (host, reviewer, Some(observer))
        } else {
            let (host, reviewer) = FileOversight::create_with_learned_text_stream_with_reserve(
                &config.store,
                config.profile.clone(),
                loaded.generation.clone(),
                RecoveryReserve::terminal(),
            )
            .unwrap();
            (host, reviewer, None)
        };
        let mut forecast =
            observer.map(|observer| prediction::Session::new(loaded.request, observer));
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        let mut wire = ActorWire::new(port);
        let mut control = Control::new(&config, loaded.request, None).unwrap();
        assert!(
            generate_selected(
                &mut driver,
                &reviewer,
                &mut config,
                &mut control,
                &deadline(),
                &mut || ElapsedTick(1000),
                forecast.as_mut()
            )
            .unwrap()
        );
        if dispatched {
            let deadline = deadline();
            submit(
                &mut driver,
                &mut wire,
                &mut config,
                &loaded,
                &deadline,
                &mut || ElapsedTick(1000),
            )
            .unwrap();
            let (reviewed, history) = super::super::continuation::review_current(
                &mut config,
                &loaded,
                &reviewer,
                driver,
                &deadline,
            );
            driver = reviewed;
            let human = super::super::continuation::approve(
                &mut driver,
                &mut config,
                &reviewer,
                loaded.request,
            );
            assert!(matches!(
                driver
                    .step_computed_from_policy_file(
                        &mut config.source,
                        || ElapsedTick(1000),
                        Some(&human)
                    )
                    .result
                    .unwrap(),
                FileDriverEvent::Dispatched { .. }
            ));
            drop(history);
        }
        let (before, bytes) = {
            let host = driver.supervisor().host().unwrap();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(numerical.position, 4);
            assert_eq!(numerical.sampled_draws, 2);
            let before = host.inspect();
            assert_eq!(before.executions, 0);
            assert!(before.stop.is_none());
            if dispatched {
                assert!(before.control.ledger.charged > 0);
            }
            let mut bytes = Vec::new();
            host.history_anchor()
                .unwrap()
                .write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES)
                .unwrap();
            (before, bytes)
        };
        drop(wire);
        drop(forecast);
        assert_eq!(cleanup(driver, 1000, 1), 0);
        private(&anchor, &bytes);
        for name in [
            "evidence.json",
            "native-roster.json",
            "helper-model.json",
            "helper-weights.safetensors",
            "helper-monitor.json",
            "helper-tokenizer.bbpe",
            "helper-sampling.json",
        ] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        fs::write(
            config.store.join("delivery.pending"),
            b"independent fleet staging",
        )
        .unwrap();
        Self {
            id,
            root,
            config,
            config_path,
            recipe: Some(path),
            anchor,
            before,
            generation: Some(loaded.generation),
        }
    }

    pub fn snapshot(&self) -> FileDeliverySnapshot {
        let Some(generation) = &self.generation else {
            return FileOversight::read_publication(&self.config.store, &self.config.profile)
                .unwrap();
        };
        if let Some(predictor) = generation.required_owned_pre_output_forecast() {
            FileOversight::read_publication_with_owned_learned_consistency(
                &self.config.store,
                &self.config.profile,
                generation,
                predictor,
            )
            .unwrap()
            .publication
        } else {
            FileOversight::read_publication_with_learned_generation(
                &self.config.store,
                &self.config.profile,
                generation,
            )
            .unwrap()
        }
    }

    pub fn image(&self) -> Vec<u8> {
        fs::read(self.config.store.join("delivery.bin")).unwrap()
    }

    pub fn files(&self) -> BTreeMap<String, Vec<u8>> {
        fs::read_dir(&self.config.store)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                assert!(entry.file_type().unwrap().is_file());
                (
                    entry.file_name().into_string().unwrap(),
                    fs::read(entry.path()).unwrap(),
                )
            })
            .collect()
    }

    // Change physical packaging only. The complete effective model remains the
    // same original generation, so recovery must accept this explicit source.
    pub fn use_source_manifest(&self) -> (PathBuf, String) {
        let path = self.recipe.as_ref().unwrap();
        let weights = self.root.0.join("weights.safetensors");
        let manifest = self.root.0.join("fleet-checkpoint-source.json");
        let text = format!(
            "{{\"schema\":\"fa.checkpoint-weights/1\",\"base\":{{\"kind\":\"single\",\"path\":\"{}\"}},\"adapter\":null,\"limits\":{{\"weight_bytes\":65536,\"weight_read_calls\":4096}}}}",
            weights.display()
        );
        private(&manifest, text.as_bytes());
        let original = fs::read_to_string(path).unwrap();
        assert!(original.contains("fa.learned-publication/1"));
        let selection = format!(
            "{{\"format\":\"checkpoint_source\",\"path\":\"{}\"}}",
            manifest.display()
        );
        let replacement = original
            .replacen("fa.learned-publication/1", "fa.learned-publication/3", 1)
            .replacen("\"sidecar\":{", "\"sidecar\":{\"priority\":[],", 1)
            .replacen(
                "\"native_review\":{",
                "\"native_review\":{\"round_ids\":[1],",
                1,
            )
            .replacen(
                &format!("\"weights\":\"{}\"", weights.display()),
                &format!("\"weights\":{selection}"),
                1,
            );
        assert_ne!(replacement, original);
        private(path, replacement.as_bytes());
        (manifest, text)
    }
}

pub(super) struct Fleet {
    pub root: Root,
    pub members: Vec<Member>,
    pub archive: Option<PathBuf>,
}
impl Fleet {
    pub fn new(members: Vec<Member>) -> Self {
        let value = Self {
            root: Root::new(),
            members,
            archive: None,
        };
        value.write(INPUT_LIMIT);
        value
    }
    pub fn coordinator(&self) -> PathBuf {
        self.root.0.join("coordinator")
    }
    pub fn manifest(&self) -> PathBuf {
        self.root.0.join("fleet.json")
    }
    pub fn document(&self, input: usize) -> String {
        let archive = self
            .archive
            .as_ref()
            .map_or("null".into(), |path| format!("\"{}\"", path.display()));
        let members = self
            .members
            .iter()
            .map(|member| {
                let anchor = if self.archive.is_none() {
                    format!(",\"history_anchor\":\"{}\"", member.anchor.display())
                } else {
                    String::new()
                };
                let recipe = member.recipe.as_ref().map_or(String::new(), |path| {
                    format!(",\"learned_recipe\":\"{}\"", path.display())
                });
                format!(
                    "{{\"id\":\"{}\",\"config\":\"{}\"{anchor}{recipe}}}",
                    member.id,
                    member.config_path.display()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            concat!(
                "{{\"schema_version\":\"fa.shutdown-fleet/3\",\"plan_archive\":{},\"operation\":\"900\",",
                "\"coordinator\":\"{}\",\"minimum_revision\":\"0\",\"max_visits\":64,",
                "\"max_head_bytes\":4194304,\"coordinator_bytes\":16777216,\"input_bytes\":{},",
                "\"runtime_ms\":10000,\"replay_events\":1,\"members\":[{}]}}"
            ),
            archive,
            self.coordinator().display(),
            input,
            members
        )
    }
    pub fn write(&self, input: usize) {
        private(&self.manifest(), self.document(input).as_bytes());
    }
    pub fn args(&self, mode: &str, extra: &[&str]) -> Vec<String> {
        [mode.to_owned(), self.manifest().display().to_string()]
            .into_iter()
            .chain(extra.iter().map(|s| (*s).to_owned()))
            .collect()
    }
    pub fn images(&self) -> Vec<Vec<u8>> {
        self.members.iter().map(Member::image).collect()
    }
    pub fn states(&self) -> Vec<FileDeliverySnapshot> {
        self.members.iter().map(Member::snapshot).collect()
    }
    pub fn files(&self) -> Vec<BTreeMap<String, Vec<u8>>> {
        self.members.iter().map(Member::files).collect()
    }
    pub fn coordinated(&self) -> Vec<u8> {
        fs::read(self.coordinator().join("delivery.bin")).unwrap()
    }

    pub fn retain_plan(&mut self) {
        let domains = self
            .members
            .iter()
            .map(|member| {
                let bytes = fs::read(&member.anchor).unwrap();
                let anchor = FileHistoryAnchor::read_trusted(
                    &mut Cursor::new(&bytes),
                    MAX_HISTORY_ANCHOR_BYTES,
                )
                .unwrap();
                FileShutdownDomain::from_history_anchor(
                    FileShutdownMember {
                        id: member.id,
                        directory: member.config.store.clone(),
                        profile: member.config.profile.clone(),
                    },
                    &anchor,
                )
                .unwrap()
            })
            .collect();
        let plan = FileShutdownPlan::new(OPERATION, domains, 64, 4_194_304).unwrap();
        let archive = self.root.0.join("fixed-plan.archive");
        private(&archive, &plan.encode().unwrap());
        self.archive = Some(archive);
        self.write(INPUT_LIMIT);
    }

    pub fn exact_input_bytes(&self) -> usize {
        assert!(self.archive.is_none());
        let mut remaining = INPUT_LIMIT;
        let mut excluded = vec![self.coordinator()];
        excluded.extend(
            self.members
                .iter()
                .map(|member| member.config.store.clone()),
        );
        for member in &self.members {
            if let Some(recipe) = &member.recipe {
                let loaded =
                    recipe::load_for_fleet(recipe, &member.config, &mut remaining, &excluded)
                        .unwrap();
                assert!(loaded.native.is_none());
            }
        }
        let assets = INPUT_LIMIT - remaining;
        // Independent file-size lower bound prevents a vacuous shared-budget
        // test if the numerical loader were to forget to debit its reads.
        let minimum_assets: usize = self
            .members
            .iter()
            .filter_map(|member| {
                member.recipe.as_ref().map(|recipe| {
                    fs::read(recipe).unwrap().len()
                        + fs::read(member.root.0.join("weights.safetensors"))
                            .unwrap()
                            .len()
                        + fs::read(member.root.0.join("tokenizer.json"))
                            .unwrap()
                            .len()
                        + fs::read(member.root.0.join("fit.fakv")).unwrap().len()
                })
            })
            .sum();
        assert!(
            assets >= minimum_assets,
            "model/tokenizer/codec bytes must be charged"
        );
        let custody: usize = self
            .members
            .iter()
            .map(|member| {
                fs::read(&member.config_path).unwrap().len()
                    + fs::read(&member.anchor).unwrap().len()
            })
            .sum();
        let mut input = assets + custody;
        loop {
            let next = assets + custody + self.document(input).len();
            if input == next {
                return input;
            }
            input = next;
        }
    }
}
