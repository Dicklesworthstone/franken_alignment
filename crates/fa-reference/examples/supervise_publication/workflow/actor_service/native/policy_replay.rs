//! Historical comparison from the original source-backed native recovery cursor.
use super::{Config, FileOversight, PublicationProfile, anchors, bootstrap, debug, operator, recipe};
use fa_reference::action::consequence::{
    delivery::persistent::{
        FileDeliverySnapshot,
        observed::stream::generated::checked::source::FileGeneratedSourceRecoveryStatus,
    },
    gate::containment::session::policy::Policy,
    policy_campaign::{PolicyReplayReport, ReplayLimits},
};
use std::path::{Path, PathBuf};

use std::{fs, io, os::unix::fs::MetadataExt};

/// Paths are operator-controlled for the duration of this read. Check existing
/// inode identities as well as canonical paths, including paths that do not yet
/// exist. A hard link or a symlink cannot turn a selected live input into an
/// independent recipe, candidate or archive.
struct ProtectedInput {
    path: PathBuf,
    inode: Option<(u64, u64)>,
}

impl ProtectedInput {
    fn resolve(path: &Path) -> Result<Self, String> {
        let path = operator::resolve(path)?;
        let inode = match fs::metadata(&path) {
            Ok(metadata) => Some((metadata.dev(), metadata.ino())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(debug(error)),
        };
        Ok(Self { path, inode })
    }

    fn check(inputs: &[Self], path: &Path) -> Result<(), String> {
        let candidate = Self::resolve(path)?;
        if inputs.iter().any(|input| {
            input.path.starts_with(&candidate.path)
                || candidate.path.starts_with(&input.path)
                || (input.inode.is_some() && input.inode == candidate.inode)
        }) {
            return Err("native replay input aliases the authority store, history anchor, or a live evidence source".into());
        }
        Ok(())
    }
}

pub(crate) struct NativePolicyReplay {
    recipe: PathBuf,
    custody: anchors::Selection,
    publication: Option<PublicationProfile>,
    protected: Vec<ProtectedInput>,
}

impl NativePolicyReplay {
    pub(crate) fn admit(args: &[String], config: &Config) -> Result<Self, String> {
        // The caller has checked the closed option grammar. Admit all declared
        // paths before reading the candidate, archive, recipe or model assets.
        let mut protected = vec![
            ProtectedInput::resolve(&config.store)?,
            ProtectedInput::resolve(config.source.path())?,
        ];
        let publication = if let Some(path) = args.get(8) {
            ProtectedInput::check(&protected, Path::new(path))?;
            let publication = PublicationProfile::read(Path::new(path))?;
            for source in publication.input_paths() {
                protected.push(ProtectedInput::resolve(source)?);
            }
            ProtectedInput::check(&protected, Path::new(path))?;
            Some(publication)
        } else {
            None
        };
        for document in [&args[1], &args[2], &args[4], &args[6]] {
            ProtectedInput::check(&protected, Path::new(document))?;
        }
        let custody = anchors::Options::from_paths(Some(args[6].clone()), None)?
            .admit(config, true)?
            .ok_or("native policy replay requires independent history custody")?;
        protected.push(ProtectedInput::resolve(Path::new(&args[6]))?);
        for document in [&args[1], &args[2], &args[4]].into_iter().chain(args.get(8)) {
            ProtectedInput::check(&protected, Path::new(document))?;
        }
        Ok(Self { recipe: PathBuf::from(&args[4]), custody, publication, protected })
    }

    pub(crate) fn read(
        self,
        config: &Config,
        candidate: Policy,
        limits: ReplayLimits,
        mut checkpoint: impl FnMut() -> Result<(), String>,
    ) -> Result<(FileDeliverySnapshot, PolicyReplayReport), String> {
        checkpoint()?;
        let inputs = recipe::load_with_path_check(
            &self.recipe,
            config.profile.delivery.scope.tenant,
            config.profile.delivery.limits.bytes,
            |path| ProtectedInput::check(&self.protected, path),
        )?;
        checkpoint()?;
        let selected = bootstrap::selection(config, &inputs, self.publication.as_ref())?;
        let mut run = FileOversight::begin_open_generated_text_stream_from_source_anchored(
            &config.store,
            config.profile.clone(),
            &inputs.decoder,
            &inputs.tokenizer,
            selected,
            self.custody.input().ok_or("missing native history anchor")?,
        ).map_err(debug)?;
        loop {
            checkpoint()?;
            let progress = run.progress();
            match progress.status {
                FileGeneratedSourceRecoveryStatus::Ready => {
                    return run.finish_policy_replay(candidate, limits).map_err(debug);
                }
                FileGeneratedSourceRecoveryStatus::Replaying => {
                    run.advance(progress.replayed_events, 1).map_err(debug)?;
                }
                _ => return Err(format!("native policy replay interrupted: {:?}", progress.status)),
            }
        }
    }
}
