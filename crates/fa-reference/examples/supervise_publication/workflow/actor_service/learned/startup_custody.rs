//! Retain acknowledged recovery cuts when live startup cannot finish.
//! These snapshots carry evidence only, never a source, observer or effect key.
use super::{Config, FileOversight, Loaded, anchors, debug};
use crate::workflow::continuation::checkpoint::{AcknowledgedOutput, Output};
use crate::workflow::continuation::guarded::mediated::profile::{History, Selection};
use fa_reference::action::consequence::delivery::persistent::observed::{
    driver::topology::{FileDriverTopology, checkpoint::TopologyCheckpoint},
    guarded::anchored::FileHistoryAnchor,
};
use std::path::Path;

/// Admit the independently selected source destination BEFORE recovery can
/// acknowledge its Fence. No next-source documents are needed for this step.
pub(super) struct FenceRetention<'a> {
    loaded: &'a Loaded,
    history: &'a History,
    source: Option<AcknowledgedOutput>,
}

impl<'a> FenceRetention<'a> {
    pub(super) fn admit(
        config: &Config,
        loaded: &'a Loaded,
        history: &'a History,
        selection: &Selection,
    ) -> Result<Self, String> {
        let source = selection
            .acknowledged_output()
            .map(|path| {
                if history.checkpoint().is_none() {
                    return Err("recovery custody requires an exact v2 source checkpoint".into());
                }
                let output = AcknowledgedOutput::admit(config, path)?;
                if loaded.anchor_output.as_deref() == Some(output.path()) {
                    return Err("history anchor and topology checkpoint outputs must be distinct new files".into());
                }
                Ok::<_, String>(output)
            })
            .transpose()?;
        Ok(Self { loaded, history, source })
    }

    /// Re-derive the exact Fence cut from the SAME healthy owner after a failed
    /// source bind. A poisoned owner or any later acknowledged event refuses;
    /// stale pre-bind evidence is never relabeled as the current source cut.
    pub(super) fn retain_error(&self, host: &FileOversight, error: String) -> String {
        let history = self.loaded.anchor_output.as_deref().map(|path| {
            let previous = self.loaded.recovery_anchor.as_ref()
                .ok_or("recovery retention requires independently held history")?;
            let anchor = host.history_anchor_after(previous).map_err(debug)?;
            Ok::<_, String>((path, anchor))
        }).transpose();
        let source = self.source.as_ref().map(|output| {
            let saved = self.history.checkpoint()
                .ok_or("recovery custody requires an exact v2 source checkpoint")?;
            let checkpoint = saved.after_recovery_fence(host).map_err(debug)?;
            Ok::<_, String>((output, checkpoint))
        }).transpose();
        // Capture both before either independent file write. A failed export
        // must never suppress the other destination or initiate another replay.
        let history = history.and_then(|saved| match saved {
            Some((path, anchor)) => anchors::export_acknowledged(&anchor, path),
            None => Ok(()),
        });
        let source = source.and_then(|saved| match saved {
            Some((output, checkpoint)) => output.retain(&checkpoint),
            None => Ok(()),
        });
        combine(error, history, source)
    }
}

/// After source binding, retain the newly acknowledged graph and its original
/// work, not the earlier Fence snapshot. The remaining startup operations only
/// inspect that owner or install local gateways, grants and listeners.
pub(super) struct Captured<'a> {
    history: Option<(&'a Path, FileHistoryAnchor)>,
    source: Option<(&'a Output, TopologyCheckpoint)>,
}

impl<'a> Captured<'a> {
    pub(super) fn capture(
        loaded: &'a Loaded,
        continuing: bool,
        host: &FileOversight,
        source: Option<&FileDriverTopology>,
        output: Option<&'a Output>,
    ) -> Result<Self, String> {
        let mut captured = Self { history: None, source: None };
        let Some(previous) = loaded.recovery_anchor.as_ref().filter(|_| continuing) else {
            return Ok(captured);
        };
        let mut failures = Vec::new();
        if let Some(path) = loaded.anchor_output.as_deref() {
            match host.history_anchor_after(previous) {
                Ok(anchor) => captured.history = Some((path, anchor)),
                Err(error) => failures.push(format!("acknowledged recovery history capture failed: {}", debug(error))),
            }
        }
        if let Some(output) = output {
            match output.capture(host, source) {
                Ok(checkpoint) => captured.source = Some((output, checkpoint)),
                Err(error) => failures.push(format!("acknowledged recovery topology capture failed: {error}")),
            }
        }
        if let (Some((_, anchor)), Some((_, checkpoint))) = (&captured.history, &captured.source) {
            if anchor != checkpoint.history_anchor() {
                return Err("acknowledged recovery archives describe different cuts; preserve the original owner and previous custody".into());
            }
        }
        if !failures.is_empty() {
            return Err(captured.retain_error(failures.join("; ")));
        }
        Ok(captured)
    }

    pub(super) fn retain_error(&self, error: String) -> String {
        let history = self.history.as_ref().map(|(path, anchor)| {
            anchors::export_acknowledged(anchor, path)
        }).transpose().map(|_| ());
        let source = self.source.as_ref().map(|(output, checkpoint)| {
            output.retain_acknowledged(checkpoint)
        }).transpose().map(|_| ());
        combine(error, history, source)
    }
}

fn combine(mut error: String, history: Result<(), String>, source: Result<(), String>) -> String {
    if let Err(retention) = history {
        error.push_str(&format!("; acknowledged recovery history retention failed: {retention}; preserve the previous anchor and original store"));
    }
    if let Err(retention) = source {
        error.push_str(&format!("; acknowledged recovery topology retention failed: {retention}; preserve previous paired custody and do not resend an effect"));
    }
    error
}
