//! Live recovery chooses either original custody or a terminal admission Stop.
//! A checked peer is retained while the SAME guarded cursor verifies every event.
use super::{
    Config, Control, Deadline, ElapsedTick, Loaded, Progress, TopologyCustody, debug,
    ready_selected_with_checkpoints, selected,
};
use super::super::{anchors, topology};
use crate::workflow::continuation::checkpoint::AcknowledgedOutput;
use fa_reference::action::consequence::delivery::persistent::observed::{
    driver::topology::checkpoint::TopologyCheckpoint,
    shutdown::recovery::FileAdmissionStoppedHistory,
};
use std::{fs, path::{Path, PathBuf}};

pub(super) fn open<F>(config: &Config, loaded: &Loaded, control: &mut Control,
    deadline: &Deadline, time: &mut F) -> Result<TopologyCustody, String>
where F: FnMut() -> ElapsedTick {
    let mut pending = false;
    let recovery = {
        let mut checkpoint = |_: Option<Progress>| {
            deadline.check(time())?;
            pending |= control.recovery_pending_peer()?;
            Ok(())
        };
        ready_selected_with_checkpoints(config, loaded, &mut checkpoint)?
    };
    // The last Ready checkpoint latches this branch BEFORE ordinary finish can
    // clean staging, append its recovery Fence or issue any live observer role.
    if pending {
        return stop(config, loaded, recovery, control, deadline, time);
    }
    recovery.finish_with_topology().map_err(debug)
}

fn stop<F>(config: &Config, loaded: &Loaded, recovery: selected::Cursor,
    control: &mut Control, deadline: &Deadline, time: &mut F) -> Result<TopologyCustody, String>
where F: FnMut() -> ElapsedTick {
    // Admit only independently selected archive destinations and the original
    // store identity. No next-source documents, helper, reviewer or owner exist.
    let retention = Retention::admit(config, loaded)?;
    let target = recovery.into_recovery_stop(control.recovery_operation()?).map_err(debug)?;
    let outcome = control.stop_recovery(target, deadline, time)?;
    let mut terminal = outcome.terminal_message();
    if let Ok(history) = &outcome.application.result {
        // A deadline or lost reply after native acknowledgment cannot suppress
        // either export. Their independent outcomes never cause a Stop retry.
        if let Err(error) = retention.retain(config, history) {
            terminal.push_str("; ");
            terminal.push_str(&error);
        }
    }
    Err(terminal)
}

struct Retention<'a> {
    history: Option<&'a Path>,
    topology: Option<SourceRetention<'a>>,
}
struct SourceRetention<'a> {
    saved: &'a TopologyCheckpoint,
    archive: AcknowledgedOutput,
    identity: PathBuf,
}
impl<'a> Retention<'a> {
    fn admit(config: &Config, loaded: &'a Loaded) -> Result<Self, String> {
        let topology = match loaded.topology.as_ref() {
            Some(topology::Selection::Recover { selection, history }) => {
                selection.acknowledged_output().map(|path| {
                    let saved = history.checkpoint()
                        .ok_or("recovery Stop custody requires an exact v2 source checkpoint")?;
                    let archive = AcknowledgedOutput::admit(config, path)?;
                    if loaded.anchor_output.as_deref() == Some(archive.path()) {
                        return Err("history anchor and topology checkpoint outputs must be distinct new files".into());
                    }
                    let identity = fs::canonicalize(&config.store).map_err(debug)?;
                    Ok::<_, String>(SourceRetention { saved, archive, identity })
                }).transpose()?
            }
            Some(topology::Selection::Create(_)) => {
                return Err("topology creation inputs cannot retain a recovery Stop".into());
            }
            None => None,
        };
        Ok(Self { history: loaded.anchor_output.as_deref(), topology })
    }

    fn retain(&self, config: &Config, stopped: &FileAdmissionStoppedHistory) -> Result<(), String> {
        let history = self.history.map(|path| {
            anchors::export_acknowledged(stopped.history_anchor(), path)
        }).transpose().map(|_| ()).map_err(|error| format!(
            "history anchor retention failed: {error}; keep the previous archive and select a new output path"));
        // The ORIGINAL terminal checkpoint method proves that only the allowed
        // admission Stop/Fence suffix follows the exact independently held cut.
        // Its unchanged source work and the new anchor remain inseparable.
        let topology = self.topology.as_ref().map(|output| {
            let checkpoint = output.saved.after_admission_stop(
                &config.profile, &output.identity, stopped).map_err(debug)?;
            output.archive.retain(&checkpoint)
        }).transpose().map(|_| ()).map_err(|error: String| format!(
            "terminal topology checkpoint retention failed: {error}; keep previous paired custody and use terminal checkpoint recovery with new output paths"));
        let failures: Vec<_> = [history, topology].into_iter().filter_map(Result::err).collect();
        if failures.is_empty() { Ok(()) } else { Err(failures.join("; ")) }
    }
}
