//! Portable data for replaying the accepted events of a reference congress.
//!
//! This verifies the existing non-cryptographic commitment oracle and protocol
//! ordering, not signatures, helper independence, honesty or salt entropy. A
//! caller must separately pin the expected session context. Invalid attempts
//! are not accepted transcript events. Missing commits/reveals stay missing.

use crate::Error;
use crate::reducer::{MAX_IDENTIFIER_BYTES, MAX_VOTES};
use crate::round::{Digest, MAX_FIELD_LEN, Round, Verdict};

pub const TRANSCRIPT_VERSION: u32 = 1;
pub const MAX_TRANSCRIPT_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitRecord {
    pub member: String,
    pub digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevealRecord {
    pub member: String,
    pub verdict: Verdict,
    pub salt: Vec<u8>,
}

/// The independent phase precedes the entire reveal phase. The frozen roster
/// is retained even when a member never commits. Vectors retain acceptance
/// order; replay neither deduplicates events nor repairs malformed transcripts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundTranscript {
    pub version: u32,
    pub round: u64,
    pub evidence_root: [u8; 32],
    pub members: Vec<String>,
    pub commits: Vec<CommitRecord>,
    pub reveals: Vec<RevealRecord>,
}

impl RoundTranscript {
    /// Reconstruct the protocol rather than trusting reported member outcomes.
    /// Collection and byte bounds are checked before constructing the round.
    pub fn replay(&self) -> Result<Round, Error> {
        if self.version != TRANSCRIPT_VERSION || self.round == 0
            || self.evidence_root == [0; 32] || self.members.is_empty()
        {
            return Err(Error::InvalidInput);
        }
        if self.members.len() > MAX_VOTES || self.commits.len() > MAX_VOTES
            || self.reveals.len() > MAX_VOTES
        {
            return Err(Error::Limit);
        }
        let mut bytes = 0_usize;
        for name in self.members.iter()
            .chain(self.commits.iter().map(|entry| &entry.member))
            .chain(self.reveals.iter().map(|entry| &entry.member))
        {
            if name.is_empty() {
                return Err(Error::InvalidInput);
            }
            if name.len() > MAX_IDENTIFIER_BYTES {
                return Err(Error::Limit);
            }
            bytes = bytes.checked_add(name.len()).ok_or(Error::Limit)?;
        }
        for reveal in &self.reveals {
            if reveal.salt.len() > MAX_FIELD_LEN {
                return Err(Error::Limit);
            }
            bytes = bytes.checked_add(reveal.salt.len()).ok_or(Error::Limit)?;
        }
        if bytes > MAX_TRANSCRIPT_BYTES {
            return Err(Error::Limit);
        }
        let mut round = Round::new(self.round, &self.evidence_root)?;
        for member in &self.members {
            round.add_member(member)?;
        }
        for entry in &self.commits {
            round.commit(&entry.member, entry.digest)?;
        }
        round.open_reveals()?;
        for entry in &self.reveals {
            round.reveal(&entry.member, entry.verdict, &entry.salt)?;
        }
        Ok(round)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::{MemberOutcome, commitment};

    fn transcript() -> RoundTranscript {
        RoundTranscript {
            version: TRANSCRIPT_VERSION,
            round: 17,
            evidence_root: [3; 32],
            members: vec!["alice".into(), "bob".into()],
            commits: vec![CommitRecord {
                member: "alice".into(),
                digest: commitment(17, "alice", &[3; 32], Verdict::Allow, b"salt").unwrap(),
            }],
            reveals: vec![RevealRecord {
                member: "alice".into(), verdict: Verdict::Allow, salt: b"salt".to_vec(),
            }],
        }
    }

    #[test]
    fn accepted_protocol_replays_without_losing_missing_members() {
        let round = transcript().replay().unwrap();
        assert_eq!(round.outcome("alice"), Ok(MemberOutcome::Revealed(Verdict::Allow)));
        assert_eq!(round.outcome("bob"), Ok(MemberOutcome::Missing));
        assert_eq!(round.member_count(), 2);
    }

    #[test]
    fn changed_reveal_salt_round_and_evidence_refuse() {
        let valid = transcript();
        assert!(valid.replay().is_ok());
        let mut changed = valid.clone();
        changed.reveals[0].verdict = Verdict::Deny;
        assert_eq!(changed.replay(), Err(Error::Binding));
        let mut changed = valid.clone();
        changed.reveals[0].salt.push(1);
        assert_eq!(changed.replay(), Err(Error::Binding));
        let mut changed = valid.clone();
        changed.round += 1;
        assert_eq!(changed.replay(), Err(Error::Binding));
        let mut changed = valid;
        changed.evidence_root[0] ^= 1;
        assert_eq!(changed.replay(), Err(Error::Binding));
    }

    #[test]
    fn missing_commit_duplicate_events_and_unknown_member_are_not_repaired() {
        let valid = transcript();
        let mut changed = valid.clone();
        changed.commits.clear();
        assert_eq!(changed.replay(), Err(Error::Missing));
        let mut changed = valid.clone();
        changed.commits.push(changed.commits[0].clone());
        assert_eq!(changed.replay(), Err(Error::Duplicate));
        let mut changed = valid.clone();
        changed.reveals.push(changed.reveals[0].clone());
        assert_eq!(changed.replay(), Err(Error::Duplicate));
        let mut changed = valid;
        changed.members.retain(|name| name != "alice");
        assert_eq!(changed.replay(), Err(Error::Missing));
    }

    #[test]
    fn collection_and_field_bounds_precede_replay_allocation() {
        let mut changed = transcript();
        changed.members = vec!["a".to_owned(); MAX_VOTES + 1];
        assert_eq!(changed.replay(), Err(Error::Limit));
        let mut changed = transcript();
        changed.reveals[0].salt = vec![0; MAX_FIELD_LEN + 1];
        assert_eq!(changed.replay(), Err(Error::Limit));
        let mut changed = transcript();
        changed.commits[0].member = "a".repeat(MAX_IDENTIFIER_BYTES + 1);
        assert_eq!(changed.replay(), Err(Error::Limit));
        let mut changed = transcript();
        changed.version += 1;
        assert_eq!(changed.replay(), Err(Error::InvalidInput));
    }
}
