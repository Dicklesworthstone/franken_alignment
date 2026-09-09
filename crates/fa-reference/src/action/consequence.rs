//! FA-104 independent consequence oracle (plan 9.8; FI-A09, FI-A16).
//!
//! These are logical control decisions, never permits or verified observations.
//! The five authority restrictions form a chain. Checkpoint restoration is a
//! separate operation with a narrowing floor, not a synonym for suspension.
//! No runtime, cryptography, persistence or model restoration is implemented.

pub mod congress;
pub mod delivery;
pub mod experiment;
pub mod gate;
pub mod oversight;

use crate::Error;

pub const CONSEQUENCE_ENCODING_V1: u8 = 1;

/// The authority-restriction lattice, in increasing order of restriction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Restriction {
    Continue = 0,
    HoldEffect = 1,
    Deny = 2,
    NarrowAuthority = 3,
    SuspendRun = 4,
}

impl Restriction {
    pub const ALL: [Self; 5] = [
        Self::Continue,
        Self::HoldEffect,
        Self::Deny,
        Self::NarrowAuthority,
        Self::SuspendRun,
    ];

    pub fn join(self, other: Self) -> Self {
        self.max(other)
    }
}

/// Exactly one primary operation. Reset deliberately has no derived Ord:
/// replacing actor state and suspending a run are not interchangeable effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Consequence {
    Continue = 0,
    HoldEffect = 1,
    Deny = 2,
    NarrowAuthority = 3,
    SuspendRun = 4,
    ResetToCheckpoint = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Requirements {
    pub authority_floor: Restriction,
    pub functional_restart_required: bool,
}

impl Consequence {
    pub const ALL: [Self; 6] = [
        Self::Continue,
        Self::HoldEffect,
        Self::Deny,
        Self::NarrowAuthority,
        Self::SuspendRun,
        Self::ResetToCheckpoint,
    ];

    pub fn requires(self) -> Requirements {
        Requirements {
            authority_floor: match self {
                Self::Continue => Restriction::Continue,
                Self::HoldEffect => Restriction::HoldEffect,
                Self::Deny => Restriction::Deny,
                Self::NarrowAuthority | Self::ResetToCheckpoint => Restriction::NarrowAuthority,
                Self::SuspendRun => Restriction::SuspendRun,
            },
            functional_restart_required: self == Self::ResetToCheckpoint,
        }
    }

    /// Versioned, exact-length reference encoding; not a decision-closure digest.
    pub fn encode(self) -> [u8; 2] {
        [CONSEQUENCE_ENCODING_V1, self as u8]
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != 2 || bytes[0] != CONSEQUENCE_ENCODING_V1 {
            return Err(Error::InvalidInput);
        }
        Self::ALL
            .get(usize::from(bytes[1]))
            .copied()
            .ok_or(Error::InvalidInput)
    }
}

impl From<Restriction> for Consequence {
    fn from(restriction: Restriction) -> Self {
        match restriction {
            Restriction::Continue => Self::Continue,
            Restriction::HoldEffect => Self::HoldEffect,
            Restriction::Deny => Self::Deny,
            Restriction::NarrowAuthority => Self::NarrowAuthority,
            Restriction::SuspendRun => Self::SuspendRun,
        }
    }
}

/// Tiny independent oracle for disqualifier dominance, not a weighted reducer.
pub fn reduce_reference(disqualifier: bool, empirical: Restriction) -> Restriction {
    if disqualifier {
        empirical.join(Restriction::Deny)
    } else {
        empirical
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    EmpiricalRecommendation,
    ExactDisqualifier,
    MandatoryAbsent,
    Contradiction,
}

/// Trusted logical inputs. A production caller must establish these facts from
/// its frozen, authenticated round and registered policy, not actor assertions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecisionInputs {
    pub empirical: Restriction,
    pub exact_disqualifier: bool,
    pub mandatory_absent: bool,
    pub contradiction: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub consequence: Consequence,
    pub rules: Vec<Rule>,
}

/// Missing mandatory evidence may impose a hold, never erase an exact denial
/// or a stronger run-level restriction. No vote can manufacture a reset.
pub fn decide(inputs: DecisionInputs) -> Decision {
    let mut restriction = reduce_reference(inputs.exact_disqualifier, inputs.empirical);
    let mut rules = vec![Rule::EmpiricalRecommendation];
    if inputs.exact_disqualifier {
        rules.push(Rule::ExactDisqualifier);
    }
    if inputs.mandatory_absent {
        restriction = restriction.join(Restriction::HoldEffect);
        rules.push(Rule::MandatoryAbsent);
    }
    if inputs.contradiction {
        restriction = restriction.join(Restriction::HoldEffect);
        rules.push(Rule::Contradiction);
    }
    Decision {
        consequence: restriction.into(),
        rules,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consequence_chain_laws() {
        for (i, a) in Restriction::ALL.into_iter().enumerate() {
            assert_eq!(a.join(a), a);
            for (j, b) in Restriction::ALL.into_iter().enumerate() {
                assert_eq!(a.cmp(&b), i.cmp(&j));
                assert_eq!(a.join(b), b.join(a));
                assert!(a <= a.join(b) && b <= a.join(b));
                for c in Restriction::ALL {
                    assert_eq!(a.join(b).join(c), a.join(b.join(c)));
                    if a <= c && b <= c {
                        assert!(a.join(b) <= c);
                    }
                    if a <= b && b <= c {
                        assert!(a <= c);
                    }
                }
            }
        }
    }

    #[test]
    fn disqualifier_yields_deny() {
        for empirical in Restriction::ALL {
            assert!(reduce_reference(true, empirical) >= Restriction::Deny);
            assert_eq!(reduce_reference(false, empirical), empirical);
        }
    }

    #[test]
    fn reset_requires_narrowing() {
        let requirements = Consequence::ResetToCheckpoint.requires();
        assert_eq!(requirements.authority_floor, Restriction::NarrowAuthority);
        assert!(requirements.functional_restart_required);
        for restriction in Restriction::ALL {
            let requirements = Consequence::from(restriction).requires();
            assert_eq!(requirements.authority_floor, restriction);
            assert!(!requirements.functional_restart_required);
        }
    }

    #[test]
    fn consequence_encoding_is_injective_and_round_trips() {
        for (tag, consequence) in Consequence::ALL.into_iter().enumerate() {
            assert_eq!(consequence.encode(), [1, tag as u8]);
            assert_eq!(Consequence::decode(&consequence.encode()), Ok(consequence));
            for other in Consequence::ALL {
                assert_eq!(consequence == other, consequence.encode() == other.encode());
            }
        }
        for invalid in [&[][..], &[1][..], &[2, 0][..], &[1, 6][..], &[1, 0, 0][..]] {
            assert_eq!(Consequence::decode(invalid), Err(Error::InvalidInput));
        }
    }

    #[test]
    fn every_missing_evidence_combination_is_monotone() {
        for empirical in Restriction::ALL {
            for exact_disqualifier in [false, true] {
                for mandatory_absent in [false, true] {
                    for contradiction in [false, true] {
                        let inputs = DecisionInputs {
                            empirical,
                            exact_disqualifier,
                            mandatory_absent,
                            contradiction,
                        };
                        let decision = decide(inputs);
                        assert_eq!(decision, decide(inputs));
                        let floor = decision.consequence.requires().authority_floor;
                        assert!(floor >= empirical);
                        if exact_disqualifier {
                            assert!(floor >= Restriction::Deny);
                        }
                        if mandatory_absent || contradiction {
                            assert!(floor >= Restriction::HoldEffect);
                        }
                        assert_ne!(decision.consequence, Consequence::ResetToCheckpoint);
                        assert_eq!(
                            decision.rules.contains(&Rule::ExactDisqualifier),
                            exact_disqualifier
                        );
                    }
                }
            }
        }
    }
}
