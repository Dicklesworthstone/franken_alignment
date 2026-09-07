//! Frozen commit--reveal reference semantics (plan §9.2, FI-A14).
//!
//! # No cryptographic claim
//!
//! This module uses a fixed, unkeyed FNV-1a-64 calculation only as a
//! deterministic comparison oracle in the std-only reference crate. It is not
//! collision resistant, preimage resistant, authenticated, or suitable for a
//! production commitment protocol. The framed fields distinguish preimages in
//! this model; they do not establish that different preimages cannot collide.
//! Production cryptography must come from an admitted audited foundation.
//!
//! The model checks transcript state only: a valid reveal compares equal to the
//! value committed for its frozen round, member, evidence root, verdict, and
//! salt. It makes no claim about salt entropy, helper honesty, independence,
//! coercion resistance, real cryptographic binding, reducers, permits, or
//! external effects.

use crate::Error;
use std::collections::BTreeMap;

/// Domain separation for this reference-only commitment preimage.
pub const COMMITMENT_DOMAIN: &[u8] = b"fa-reference/congress/round/commitment/v1";
pub const MAX_MEMBERS: usize = 256;
pub const MAX_FIELD_LEN: usize = 4096;

/// A non-cryptographic, reference-only comparison value.
pub type Digest = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    Allow,
    Hold,
    Deny,
    Abstain,
}

impl Verdict {
    const fn canonical_tag(self) -> u8 {
        match self {
            Self::Allow => 1,
            Self::Hold => 2,
            Self::Deny => 3,
            Self::Abstain => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Commit,
    Reveal,
}

/// A committed member without a valid reveal is missing, not abstaining.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberOutcome {
    Missing,
    Revealed(Verdict),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MemberSlot {
    commitment: Option<Digest>,
    outcome: MemberOutcome,
}

/// One fixed-membership transcript. Membership freezes when the first
/// commitment arrives, before any commitment may influence later membership.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Round {
    id: u64,
    evidence_root: Vec<u8>,
    phase: Phase,
    membership_frozen: bool,
    members: BTreeMap<String, MemberSlot>,
}

impl Round {
    pub fn new(id: u64, evidence_root: &[u8]) -> Result<Self, Error> {
        validate_nonempty_field(evidence_root)?;
        Ok(Self {
            id,
            evidence_root: evidence_root.to_vec(),
            phase: Phase::Commit,
            membership_frozen: false,
            members: BTreeMap::new(),
        })
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn evidence_root(&self) -> &[u8] {
        &self.evidence_root
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    pub fn add_member(&mut self, member: &str) -> Result<(), Error> {
        if self.phase != Phase::Commit || self.membership_frozen {
            return Err(Error::WrongState);
        }
        validate_nonempty_field(member.as_bytes())?;
        if self.members.contains_key(member) {
            return Err(Error::Duplicate);
        }
        if self.members.len() >= MAX_MEMBERS {
            return Err(Error::Limit);
        }
        self.members.insert(
            member.to_owned(),
            MemberSlot {
                commitment: None,
                outcome: MemberOutcome::Missing,
            },
        );
        Ok(())
    }

    pub fn commit(&mut self, member: &str, digest: Digest) -> Result<(), Error> {
        if self.phase != Phase::Commit {
            return Err(Error::WrongState);
        }
        let slot = self.members.get_mut(member).ok_or(Error::Missing)?;
        if slot.commitment.is_some() {
            return Err(Error::Duplicate);
        }
        self.membership_frozen = true;
        slot.commitment = Some(digest);
        Ok(())
    }

    /// Close the independent phase. No evidence, identity, or membership
    /// mutation API exists after this transition.
    pub fn open_reveals(&mut self) -> Result<(), Error> {
        if self.phase != Phase::Commit {
            return Err(Error::WrongState);
        }
        if self.members.is_empty() {
            return Err(Error::Missing);
        }
        self.membership_frozen = true;
        self.phase = Phase::Reveal;
        Ok(())
    }

    /// Verify and record exactly one reveal. A rejected or second reveal leaves
    /// a first valid result intact.
    pub fn reveal(&mut self, member: &str, verdict: Verdict, salt: &[u8]) -> Result<(), Error> {
        if self.phase != Phase::Reveal {
            return Err(Error::WrongState);
        }
        let expected = self
            .members
            .get(member)
            .ok_or(Error::Missing)?
            .commitment
            .ok_or(Error::Missing)?;
        let actual = commitment(self.id, member, &self.evidence_root, verdict, salt)?;
        if actual != expected {
            return Err(Error::Binding);
        }
        let slot = self.members.get_mut(member).ok_or(Error::Missing)?;
        if slot.outcome != MemberOutcome::Missing {
            return Err(Error::Duplicate);
        }
        slot.outcome = MemberOutcome::Revealed(verdict);
        Ok(())
    }

    pub fn outcome(&self, member: &str) -> Result<MemberOutcome, Error> {
        self.members
            .get(member)
            .map(|slot| slot.outcome)
            .ok_or(Error::Missing)
    }

    /// Includes every frozen member, including members still marked `Missing`.
    pub fn outcomes(&self) -> BTreeMap<&str, MemberOutcome> {
        self.members
            .iter()
            .map(|(member, slot)| (member.as_str(), slot.outcome))
            .collect()
    }
}

/// Construct the reference-only comparison value for a prospective reveal.
/// Each field is length-framed to prevent concatenation ambiguity.
pub fn commitment(
    round_id: u64,
    member: &str,
    evidence_root: &[u8],
    verdict: Verdict,
    salt: &[u8],
) -> Result<Digest, Error> {
    validate_nonempty_field(member.as_bytes())?;
    validate_nonempty_field(evidence_root)?;
    validate_field_len(salt)?;

    let mut preimage = Vec::with_capacity(
        (COMMITMENT_DOMAIN.len() + member.len() + evidence_root.len() + salt.len()) + (6 * 8) + 9,
    );
    frame(&mut preimage, COMMITMENT_DOMAIN);
    frame(&mut preimage, &round_id.to_be_bytes());
    frame(&mut preimage, member.as_bytes());
    frame(&mut preimage, evidence_root);
    frame(&mut preimage, &[verdict.canonical_tag()]);
    frame(&mut preimage, salt);
    Ok(fnv1a64(&preimage))
}

fn validate_nonempty_field(field: &[u8]) -> Result<(), Error> {
    if field.is_empty() {
        return Err(Error::InvalidInput);
    }
    validate_field_len(field)
}

fn validate_field_len(field: &[u8]) -> Result<(), Error> {
    if field.len() > MAX_FIELD_LEN {
        return Err(Error::Limit);
    }
    Ok(())
}

fn frame(preimage: &mut Vec<u8>, field: &[u8]) {
    preimage.extend_from_slice(&(field.len() as u64).to_be_bytes());
    preimage.extend_from_slice(field);
}

fn fnv1a64(bytes: &[u8]) -> Digest {
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for &byte in bytes {
        digest ^= u64::from(byte);
        digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
    }
    digest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round() -> Round {
        let mut round = Round::new(7, b"evidence-root-a").unwrap();
        round.add_member("alice").unwrap();
        round.add_member("bob").unwrap();
        round
    }

    fn alice_commitment(round: &Round, verdict: Verdict, salt: &[u8]) -> Digest {
        commitment(round.id(), "alice", round.evidence_root(), verdict, salt).unwrap()
    }

    #[test]
    fn commitment_binds_verdict() {
        let digest = commitment(7, "alice", b"evidence-root-a", Verdict::Allow, b"salt-a").unwrap();
        assert_ne!(
            digest,
            commitment(8, "alice", b"evidence-root-a", Verdict::Allow, b"salt-a").unwrap()
        );
        assert_ne!(
            digest,
            commitment(7, "bob", b"evidence-root-a", Verdict::Allow, b"salt-a").unwrap()
        );
        assert_ne!(
            digest,
            commitment(7, "alice", b"evidence-root-b", Verdict::Allow, b"salt-a").unwrap()
        );
        assert_ne!(
            digest,
            commitment(7, "alice", b"evidence-root-a", Verdict::Deny, b"salt-a").unwrap()
        );
        assert_ne!(
            digest,
            commitment(7, "alice", b"evidence-root-a", Verdict::Allow, b"salt-b").unwrap()
        );
    }

    #[test]
    fn valid_reveals_are_recorded_for_the_frozen_members() {
        let mut round = round();
        let alice = alice_commitment(&round, Verdict::Allow, b"salt-a");
        let bob = commitment(
            round.id(),
            "bob",
            round.evidence_root(),
            Verdict::Hold,
            b"salt-b",
        )
        .unwrap();
        round.commit("alice", alice).unwrap();
        round.commit("bob", bob).unwrap();
        round.open_reveals().unwrap();
        round.reveal("alice", Verdict::Allow, b"salt-a").unwrap();
        round.reveal("bob", Verdict::Hold, b"salt-b").unwrap();
        assert_eq!(
            round.outcome("alice"),
            Ok(MemberOutcome::Revealed(Verdict::Allow))
        );
        assert_eq!(
            round.outcome("bob"),
            Ok(MemberOutcome::Revealed(Verdict::Hold))
        );
    }

    #[test]
    fn conflicting_reveal_rejected() {
        let mut round = round();
        let digest = alice_commitment(&round, Verdict::Allow, b"salt-a");
        round.commit("alice", digest).unwrap();
        round.open_reveals().unwrap();
        round.reveal("alice", Verdict::Allow, b"salt-a").unwrap();
        let before = round.outcome("alice");
        assert_eq!(
            round.reveal("alice", Verdict::Deny, b"other-salt"),
            Err(Error::Binding)
        );
        assert_eq!(round.outcome("alice"), before);
    }

    #[test]
    fn missing_reveal_is_missing_not_abstain() {
        let mut round = round();
        let alice = alice_commitment(&round, Verdict::Abstain, b"salt-a");
        let bob = commitment(
            round.id(),
            "bob",
            round.evidence_root(),
            Verdict::Hold,
            b"salt-b",
        )
        .unwrap();
        round.commit("alice", alice).unwrap();
        round.commit("bob", bob).unwrap();
        round.open_reveals().unwrap();
        round.reveal("alice", Verdict::Abstain, b"salt-a").unwrap();
        assert_eq!(
            round.outcome("alice"),
            Ok(MemberOutcome::Revealed(Verdict::Abstain))
        );
        assert_eq!(round.outcome("bob"), Ok(MemberOutcome::Missing));
        assert_eq!(round.outcomes().len(), 2);
        assert_eq!(round.outcomes().get("bob"), Some(&MemberOutcome::Missing));
    }

    #[test]
    fn membership_frozen_after_open() {
        let mut round = round();
        let digest = alice_commitment(&round, Verdict::Allow, b"salt-a");
        round.commit("alice", digest).unwrap();
        assert_eq!(round.add_member("mallory"), Err(Error::WrongState));
        assert_eq!(round.member_count(), 2);
        round.open_reveals().unwrap();
        assert_eq!(round.add_member("carol"), Err(Error::WrongState));
        assert_eq!(round.commit("bob", 1), Err(Error::WrongState));
    }

    #[test]
    fn invalid_reveal_and_duplicate_commit_do_not_mutate_member_slot() {
        let mut round = round();
        let digest = alice_commitment(&round, Verdict::Allow, b"salt-a");
        round.commit("alice", digest).unwrap();
        assert_eq!(round.commit("alice", digest), Err(Error::Duplicate));
        round.open_reveals().unwrap();
        assert_eq!(
            round.reveal("alice", Verdict::Allow, b"wrong-salt"),
            Err(Error::Binding)
        );
        assert_eq!(round.outcome("alice"), Ok(MemberOutcome::Missing));
        round.reveal("alice", Verdict::Allow, b"salt-a").unwrap();
        let before_duplicate = round.clone();
        assert_eq!(
            round.reveal("alice", Verdict::Allow, b"salt-a"),
            Err(Error::Duplicate)
        );
        assert_eq!(round, before_duplicate);
    }

    #[test]
    fn commit_after_reveals_open_is_rejected() {
        let mut round = round();
        round.open_reveals().unwrap();
        assert_eq!(round.commit("alice", 1), Err(Error::WrongState));
        assert_eq!(
            round.reveal("alice", Verdict::Allow, b"salt-a"),
            Err(Error::Missing)
        );
    }

    #[test]
    fn the_domain_separator_is_part_of_the_preimage() {
        let mut framed = Vec::new();
        frame(&mut framed, COMMITMENT_DOMAIN);
        frame(&mut framed, &1_u64.to_be_bytes());
        frame(&mut framed, b"alice");
        frame(&mut framed, b"evidence-root-a");
        frame(&mut framed, &[Verdict::Allow.canonical_tag()]);
        frame(&mut framed, b"salt-a");

        // Positive: the commitment is exactly this preimage, domain tag first,
        // every field length-framed in this order. Reordering a field, dropping
        // a length prefix or moving the tag fails here.
        assert_eq!(
            commitment(1, "alice", b"evidence-root-a", Verdict::Allow, b"salt-a").unwrap(),
            fnv1a64(&framed)
        );

        // Causal negative: the same fields without the domain tag hash to a
        // different value, so a commitment minted for this context cannot be
        // reused by another that frames its own tag. Every other assertion in
        // this module compares two commitments, and the tag is a shared
        // constant prefix, so deleting it from the preimage would leave all of
        // them green. This is the only assertion that catches it.
        let mut unseparated = Vec::new();
        frame(&mut unseparated, &1_u64.to_be_bytes());
        frame(&mut unseparated, b"alice");
        frame(&mut unseparated, b"evidence-root-a");
        frame(&mut unseparated, &[Verdict::Allow.canonical_tag()]);
        frame(&mut unseparated, b"salt-a");
        assert_ne!(fnv1a64(&framed), fnv1a64(&unseparated));
    }

    #[test]
    fn maximum_member_and_field_bounds_are_exact_and_reject_without_mutation() {
        let exact_root = vec![b'r'; MAX_FIELD_LEN];
        assert!(Round::new(1, &exact_root).is_ok());
        assert_eq!(
            Round::new(1, &vec![b'r'; MAX_FIELD_LEN + 1]),
            Err(Error::Limit)
        );

        let mut round = Round::new(2, b"root").unwrap();
        for number in 0..MAX_MEMBERS {
            round.add_member(&format!("member-{number}")).unwrap();
        }
        assert_eq!(round.member_count(), MAX_MEMBERS);
        let before_extra_member = round.clone();
        assert_eq!(round.add_member("one-too-many"), Err(Error::Limit));
        assert_eq!(round, before_extra_member);

        let exact_salt = vec![b's'; MAX_FIELD_LEN];
        assert!(commitment(2, "member-0", b"root", Verdict::Allow, &exact_salt).is_ok());
        assert_eq!(
            commitment(
                2,
                "member-0",
                b"root",
                Verdict::Allow,
                &vec![b's'; MAX_FIELD_LEN + 1]
            ),
            Err(Error::Limit)
        );
    }
}
