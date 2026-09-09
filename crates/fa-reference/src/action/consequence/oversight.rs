//! Complete, action-bound helper input views for the reference effect path.
//! Provider capture and helper inference remain caller-trusted observations.

mod packet;
pub use packet::{CommitteeContract, CommitteeInput, HelperContract, MAX_COMMITTEE_BYTES, action_frame};
