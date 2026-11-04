//! Closed exact native capture coordinates, with no inferred current values.
use super::{FileLearnedStreamCheckpointRequest, REQUEST_BYTES, debug, fields::Fields};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::checkpoint::FileLearnedCheckpointInfo;
use fa_reference::strict_json::{self, Limits};

fn number(fields: &mut Fields, name: &str) -> Result<u64, String> {
    let text = fields.text(name)?;
    if text.is_empty() || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("{name} must be a canonical unsigned decimal string"));
    }
    text.parse().map_err(|_| format!("{name} exceeds the unsigned 64-bit range"))
}

pub(super) fn decode(bytes: &[u8]) -> Result<FileLearnedStreamCheckpointRequest, String> {
    let json = strict_json::parse(bytes, Limits { max_bytes: REQUEST_BYTES,
        max_depth: 2, max_items: 32, max_string_bytes: 64 }).map_err(debug)?;
    let mut fields = Fields::new(json)?;
    if fields.text("schema")? != "fa.learned-stream-checkpoint/1" {
        return Err("unsupported stream-checkpoint schema".into());
    }
    let revision = number(&mut fields, "expected_journal_revision")?;
    let checkpoint = FileLearnedCheckpointInfo {
        checkpoint: number(&mut fields, "checkpoint")?,
        actor_revision: number(&mut fields, "expected_actor_revision")?,
        authority_epoch: number(&mut fields, "expected_authority_epoch")?,
        control_sequence: number(&mut fields, "expected_control_sequence")?,
        stream: number(&mut fields, "expected_stream")?,
        position: number(&mut fields, "expected_position")?,
        sampled_draws: number(&mut fields, "expected_sampled_draws")?,
    };
    fields.end()?;
    FileLearnedStreamCheckpointRequest::new(revision, checkpoint).map_err(debug)
}
