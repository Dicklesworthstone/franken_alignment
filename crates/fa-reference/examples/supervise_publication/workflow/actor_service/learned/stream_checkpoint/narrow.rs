//! Independently retained checkpoint and exact predecessor for prompt containment.
//! No coordinate, evidence binding or target version is inferred from live state.
use super::{FileLearnedStreamNarrowRequest, REQUEST_BYTES, debug, fields::{self, Fields}};
use fa_reference::action::{MAX_ATTEMPTS, ResolvedTarget};
use fa_reference::action::consequence::{
    delivery::persistent::observed::decoder::learned::checkpoint::{
        FileLearnedCheckpointInfo, FileLearnedStreamNarrowControl,
    },
    gate::ReviewBinding,
};
use fa_reference::strict_json::{self, Limits};

fn number(fields: &mut Fields, name: &str) -> Result<u64, String> {
    let value = fields.text(name)?;
    if value.is_empty() || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("{name} must be a canonical unsigned decimal string"));
    }
    value.parse().map_err(|_| format!("{name} exceeds the unsigned 64-bit range"))
}

pub(super) fn decode(bytes: &[u8]) -> Result<FileLearnedStreamNarrowRequest, String> {
    let json = strict_json::parse(bytes, Limits { max_bytes: REQUEST_BYTES,
        max_depth: 6, max_items: 16_384, max_string_bytes: 64 }).map_err(debug)?;
    let mut root = Fields::new(json)?;
    if root.text("schema")? != "fa.learned-stream-narrow/1" {
        return Err("unsupported stream-checkpoint narrowing schema".into());
    }
    let revision = number(&mut root, "expected_journal_revision")?;
    let mut saved = root.object("checkpoint")?;
    let checkpoint = FileLearnedCheckpointInfo {
        checkpoint: number(&mut saved, "checkpoint")?,
        actor_revision: number(&mut saved, "actor_revision")?,
        authority_epoch: number(&mut saved, "authority_epoch")?,
        control_sequence: number(&mut saved, "control_sequence")?,
        stream: number(&mut saved, "stream")?,
        position: number(&mut saved, "position")?,
        sampled_draws: number(&mut saved, "sampled_draws")?,
    };
    saved.end()?;
    let mut control = root.object("control")?;
    let operation = number(&mut control, "operation")?;
    let expected_control_sequence = number(&mut control, "expected_control_sequence")?;
    let expected_actor_revision = number(&mut control, "expected_actor_revision")?;
    let expected_authority_epoch = number(&mut control, "expected_authority_epoch")?;
    let mut binding = control.object("binding")?;
    let round = number(&mut binding, "round")?;
    let reducer_generation = number(&mut binding, "reducer_generation")?;
    let evidence_root = fields::hex(&binding.text("evidence_root_hex")?, 32)?.try_into()
        .map_err(|_| "narrowing evidence root must contain exactly 32 bytes")?;
    binding.end()?;
    let retained_targets = control.array("retained_targets", MAX_ATTEMPTS)?.into_iter()
        .map(|target| {
            let mut target = Fields::new(target)?;
            let value = ResolvedTarget {
                adapter: number(&mut target, "adapter")?,
                object: number(&mut target, "object")?,
                contract_version: number(&mut target, "contract_version")?,
                expected_version: number(&mut target, "expected_version")?,
                generation: number(&mut target, "generation")?,
            };
            target.end()?;
            Ok::<_, String>(value)
        }).collect::<Result<Vec<_>, _>>()?;
    control.end()?;
    root.end()?;
    FileLearnedStreamNarrowRequest::new(revision, checkpoint, FileLearnedStreamNarrowControl {
        operation, expected_control_sequence, expected_actor_revision, expected_authority_epoch,
        binding: ReviewBinding { round, reducer_generation, evidence_root }, retained_targets,
    }).map_err(debug)
}
