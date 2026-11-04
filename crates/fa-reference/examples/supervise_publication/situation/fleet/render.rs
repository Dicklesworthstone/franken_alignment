//! Wrap the ORIGINAL per-journal renderer; never reinterpret its accounting.
use fa_reference::action::consequence::delivery::persistent::observed::replay::situation::fleet::{
    FileSituationFleetPage, FileSituationReadFailure,
};
use fa_reference::action::consequence::oversight::actor::Knowledge;

pub(super) const MAX_PAGE_BYTES: usize = super::super::render::MAX_PAGE_BYTES + 1024;

pub(super) fn page(page: &FileSituationFleetPage) -> Result<Vec<u8>, String> {
    let (state, failure, inner) = match (&page.observation, page.failure) {
        (Knowledge::Known { value, basis }, None) => {
            let mut bytes = super::super::render::page(value, *basis)?;
            if bytes.pop() != Some(b'\n') { return Err("invalid native situation framing".into()); }
            ("known", "null", bytes)
        }
        (Knowledge::Unknown { .. }, Some(reason)) => {
            let failure = match reason {
                FileSituationReadFailure::InputUnavailable => "\"input_unavailable\"",
                FileSituationReadFailure::ReplayRejected => "\"replay_rejected\"",
                FileSituationReadFailure::CaptureRejected => "\"capture_rejected\"",
                FileSituationReadFailure::Interrupted => "\"interrupted\"",
            };
            ("unknown", failure, b"null".to_vec())
        }
        _ => return Err("inconsistent fleet member observation".into()),
    };
    let c = page.coverage;
    let mut bytes = format!(concat!(
        "{{\"schema\":\"fa.publication-situation-fleet-page/1\",",
        "\"historical\":true,\"atomic_cut\":false,\"roster\":\"declared_not_authenticated\",",
        "\"member\":\"{}\",\"coverage\":{{\"declared_members\":\"{}\",\"known_members\":\"{}\",\"unknown_members\":\"{}\"}},",
        "\"all_members_known\":{},\"enumeration_complete\":{},",
        "\"member_state\":\"{}\",\"failure\":{},\"situation\":"),
        page.member, c.declared_members, c.known_members, c.unknown_members,
        c.all_members_known(), page.next.is_none(), state, failure).into_bytes();
    bytes.extend_from_slice(&inner);
    bytes.extend_from_slice(b"}\n");
    if bytes.len() > MAX_PAGE_BYTES { return Err("fleet situation page exceeds byte bound".into()); }
    Ok(bytes)
}
