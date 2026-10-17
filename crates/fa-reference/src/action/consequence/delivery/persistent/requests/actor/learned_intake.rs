//! Supervisor-selected limits for one source-only learned stream request.
//! Selection is not a permit, a source snapshot, or actor-supplied output.
use super::{FileLearnedTextStreamActorPort, LearnedTextRelease};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::actor::{ActorError, ActorProposal};
use crate::Error;

/// Independent intake scope for a dedicated learned actor session. The client
/// may choose a shorter deadline, never another request or release kind. Once
/// recorded, the original gateway additionally requires the exact old deadline.
/// This copyable configuration cannot authorize, review, or publish anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedStreamAdmission {
    request: u64,
    release: LearnedTextRelease,
    latest_deadline: ElapsedTick,
}
impl LearnedStreamAdmission {
    pub fn new(request: u64, release: LearnedTextRelease, latest_deadline: ElapsedTick)
        -> Result<Self, Error>
    {
        if request == 0 || latest_deadline.0 == 0 { return Err(Error::InvalidInput); }
        Ok(Self { request, release, latest_deadline })
    }
    pub fn request(self) -> u64 { self.request }

    // Closed canonical encoding, not another wire parser. Check this before
    // learned-source preflight, policy I/O, time, or snapshot consumption.
    pub(super) fn check(self, request: u64, proposal: &ActorProposal) -> Result<(), ActorError> {
        if request != self.request { return Err(ActorError::Withheld); }
        if proposal.deadline.0 == 0 || proposal.deadline > self.latest_deadline {
            return Err(ActorError::MalformedProposal);
        }
        let expected = FileLearnedTextStreamActorPort::encode_release(request,
            self.release, proposal.deadline)?;
        if proposal != &expected { return Err(ActorError::MalformedProposal); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_binds_every_envelope_field_and_rejects_other_keys_and_release_kinds() {
        let selection = LearnedStreamAdmission::new(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
        for tick in [1, 99, 100] {
            let proposal = FileLearnedTextStreamActorPort::encode_release(71, LearnedTextRelease::Message, ElapsedTick(tick)).unwrap();
            assert_eq!(selection.check(71, &proposal), Ok(()));
            assert_eq!(selection.check(72, &proposal), Err(ActorError::Withheld));
            for field in 0..7 {
                let mut changed = proposal.clone();
                match field {
                    0 => changed.target.adapter += 1, 1 => changed.target.object += 1,
                    2 => changed.target.contract_version += 1, 3 => changed.target.expected_version += 1,
                    4 => changed.target.generation += 1, 5 => changed.units += 1,
                    _ => changed.expected_policy_epoch += 1,
                }
                assert_eq!(selection.check(71, &changed), Err(ActorError::MalformedProposal));
            }
            for byte in 0..proposal.payload.len() {
                let mut changed = proposal.clone(); changed.payload[byte] ^= 1;
                assert_eq!(selection.check(71, &changed), Err(ActorError::MalformedProposal));
            }
            let mut changed = proposal.clone(); changed.payload.push(0);
            assert_eq!(selection.check(71, &changed), Err(ActorError::MalformedProposal));
            let mut changed = proposal; changed.payload.pop();
            assert_eq!(selection.check(71, &changed), Err(ActorError::MalformedProposal));
        }
        let finish = FileLearnedTextStreamActorPort::encode_release(71, LearnedTextRelease::Finish, ElapsedTick(100)).unwrap();
        assert_eq!(selection.check(71, &finish), Err(ActorError::MalformedProposal));
        assert_eq!(LearnedStreamAdmission::new(71, LearnedTextRelease::Finish, ElapsedTick(100)).unwrap().check(71, &finish), Ok(()));
    }
    #[test]
    fn admission_deadline_is_nonzero_bounded_and_never_silently_renewed() {
        assert_eq!(LearnedStreamAdmission::new(0, LearnedTextRelease::Message, ElapsedTick(1)), Err(Error::InvalidInput));
        assert_eq!(LearnedStreamAdmission::new(1, LearnedTextRelease::Message, ElapsedTick(0)), Err(Error::InvalidInput));
        let selection = LearnedStreamAdmission::new(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
        assert_eq!(selection.request(), 71);
        for tick in [0, 101, u64::MAX] {
            let proposal = FileLearnedTextStreamActorPort::encode_release(71, LearnedTextRelease::Message, ElapsedTick(tick)).unwrap();
            assert_eq!(selection.check(71, &proposal), Err(ActorError::MalformedProposal));
        }
    }
}
