//! New independent archives for exactly the native acknowledged disposition.
use super::{Config, Loaded, anchors, topology};
use crate::{config::debug, workflow::continuation::checkpoint::AcknowledgedOutput};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::recovery::FileAbandonedHistory;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) struct Outputs {
    history: PathBuf,
    source: Option<AcknowledgedOutput>,
    identity: PathBuf,
}

pub(super) struct Retention {
    pub history: Result<(), String>,
    pub source: Option<Result<(), String>>,
}

impl Retention {
    pub(super) fn failures(self) -> Vec<String> {
        let mut errors = Vec::new();
        if let Err(error) = self.history {
            errors.push(format!("history anchor retention failed: {error}"));
        }
        if let Some(Err(error)) = self.source {
            errors.push(format!("topology checkpoint retention failed: {error}"));
        }
        errors
    }
}

pub(super) fn admit(
    config: &Config,
    loaded: &Loaded,
    path: Option<&Path>,
) -> Result<Outputs, String> {
    let anchor = loaded
        .recovery_anchor
        .as_ref()
        .ok_or("learned abandonment requires an independent input history anchor")?;
    let history = loaded
        .anchor_output
        .as_ref()
        .ok_or("learned abandonment requires a new retained history anchor output")?;
    let source = match (topology::history(loaded)?, path) {
        (None, None) => None,
        (Some(selected), Some(path)) => {
            let saved = selected.checkpoint().ok_or(
                "learned abandonment requires an exact v2 topology checkpoint; manual source minima cannot establish custody")?;
            if saved.history_anchor() != anchor {
                return Err("abandonment source checkpoint and input history anchor describe different cuts".into());
            }
            let output = AcknowledgedOutput::admit(config, path)?;
            if output.path() == history {
                return Err(
                    "abandonment history and source checkpoint outputs must be distinct new files"
                        .into(),
                );
            }
            Some(output)
        }
        _ => {
            return Err(
                "learned abandonment requires paired topology profile and new checkpoint output"
                    .into(),
            );
        }
    };
    Ok(Outputs {
        history: history.clone(),
        source,
        identity: fs::canonicalize(&config.store).map_err(debug)?,
    })
}

impl Outputs {
    /// Capturing source custody and retaining either output are independent.
    /// Neither failure prevents the other output or the final bounded report.
    pub(super) fn retain(
        &self,
        config: &Config,
        loaded: &Loaded,
        result: &FileAbandonedHistory,
    ) -> Retention {
        let captured = self.source.as_ref().map(|_| {
            let selected = topology::history(loaded)?.ok_or("missing original topology custody")?;
            let saved = selected.checkpoint().ok_or("missing original source checkpoint")?;
            let next = saved.after_abandoned_history(&config.profile, &self.identity, result).map_err(debug)?;
            if next.history_anchor() != result.history_anchor() {
                return Err("abandonment source checkpoint and acknowledged history describe different cuts".into());
            }
            Ok::<_, String>(next)
        });
        let history = anchors::export_acknowledged(result.history_anchor(), &self.history);
        let source = self.source.as_ref().map(|output| match captured {
            Some(Ok(saved)) => output.retain(&saved),
            Some(Err(error)) => Err(format!("checkpoint capture failed: {error}")),
            None => Err("missing selected source custody".into()),
        });
        Retention { history, source }
    }
}
