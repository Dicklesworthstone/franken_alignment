//! Epistemic wrapper `Knowledge<T>` combinators, layer-boundary serializer checks,
//! and permit-path fail-closed admissibility (`FA-133`, plan §6.7, §6.8, §17.2,
//! `FA-INV-004`, `FA-INV-039`).
//!
//! Every safety-relevant value crossing a layer boundary or reaching an agent
//! surface must be wrapped in [`Knowledge`]. Bare scalars (`bool`, `u64`, raw strings)
//! do not implement [`SafetyRelevant`] and therefore cannot be serialized as
//! safety-relevant agent fields:
//!
//! ```compile_fail
//! use fa_reference::knowledge_surface::{AgentSurfaceSerializer, SafetyRelevant};
//! use fa_reference::observation_algebra::Knowledge;
//!
//! // A bare `bool` is not `SafetyRelevant`, so attempting to serialize even
//! // a bare boolean or an unwrapped scalar through the safety serializer fails to compile.
//! let bare_safe: bool = true;
//! let _ = AgentSurfaceSerializer::serialize_safety_field("safe", &bare_safe);
//! ```
//!
//! Even wrapping a bare `bool` inside `Knowledge<bool>` is rejected at compile time
//! because `bool` does not implement [`SafetyRelevant`]:
//!
//! ```compile_fail
//! use fa_reference::knowledge_surface::AgentSurfaceSerializer;
//! use fa_reference::observation_algebra::{Knowledge, UnknownReason};
//!
//! let wrapped_bare_bool: Knowledge<bool> = Knowledge::unknown(UnknownReason::Incomplete);
//! let _ = AgentSurfaceSerializer::serialize_safety_field("safe", &wrapped_bare_bool);
//! ```

use std::fmt::Write as _;

use crate::{
    Error,
    observation_algebra::{
        AuthorityKind, Basis, BasisIdentity, ClaimClass, ClosedDomain, Knowledge, ProofStates,
        ScopedProof, UnknownReason,
    },
    product_frontier::{FrontierRequirement, FrontierStage, TrustedClosingMarker},
    witness::DomainProjection,
};

pub const KNOWLEDGE_ENCODING_V1: u8 = 1;

/// Rank of a [`ClaimClass`] on the epistemic strength chain (`plan §2.4`).
/// Higher rank means stronger epistemic assurance.
pub const fn claim_class_rank(class: ClaimClass) -> u8 {
    match class {
        ClaimClass::Hypothesis => 1,
        ClaimClass::Slo => 2,
        ClaimClass::Benchmark => 3,
        ClaimClass::Statistical => 4,
        ClaimClass::BoundedModel => 5,
        ClaimClass::Proof => 6,
        ClaimClass::Invariant => 7,
    }
}

/// Conservative meet of two [`ClaimClass`] values (returns the weaker class).
pub const fn weaker_claim_class(lhs: ClaimClass, rhs: ClaimClass) -> ClaimClass {
    if claim_class_rank(lhs) <= claim_class_rank(rhs) {
        lhs
    } else {
        rhs
    }
}

fn join_scoped_proof(lhs: ScopedProof, rhs: ScopedProof) -> ScopedProof {
    match (lhs, rhs) {
        (
            ScopedProof::Declared {
                scope_id: s1,
                generation: g1,
            },
            ScopedProof::Declared {
                scope_id: s2,
                generation: g2,
            },
        ) if s1 == s2 && g1 == g2 => ScopedProof::Declared {
            scope_id: s1,
            generation: g1,
        },
        (ScopedProof::Unsupported { scope_id }, _) | (_, ScopedProof::Unsupported { scope_id }) => {
            ScopedProof::Unsupported { scope_id }
        }
        (
            ScopedProof::Gapped {
                scope_id,
                first_missing,
            },
            _,
        )
        | (
            _,
            ScopedProof::Gapped {
                scope_id,
                first_missing,
            },
        ) => ScopedProof::Gapped {
            scope_id,
            first_missing,
        },
        (
            ScopedProof::Expired {
                scope_id,
                generation,
            },
            _,
        )
        | (
            _,
            ScopedProof::Expired {
                scope_id,
                generation,
            },
        ) => ScopedProof::Expired {
            scope_id,
            generation,
        },
        (
            ScopedProof::Declared {
                scope_id,
                generation,
            },
            ScopedProof::Declared { .. },
        ) => ScopedProof::Expired {
            scope_id,
            generation,
        },
    }
}

/// Conservative combination of two [`Basis`] records at the same generation and epoch.
pub fn join_basis_conservative(lhs: Basis, rhs: Basis) -> Result<Basis, UnknownReason> {
    let l_id = lhs.identity();
    let r_id = rhs.identity();
    if l_id.semantic_generation != r_id.semantic_generation
        || l_id.policy_epoch != r_id.policy_epoch
        || l_id.model_epoch != r_id.model_epoch
        || l_id.tokenizer_epoch != r_id.tokenizer_epoch
        || l_id.codec_epoch != r_id.codec_epoch
    {
        return Err(UnknownReason::IncompatibleGeneration);
    }

    let joined_identity = BasisIdentity {
        subject_id: l_id.subject_id,
        question_id: l_id.question_id,
        profile_id: l_id.profile_id,
        semantic_generation: l_id.semantic_generation,
        policy_epoch: l_id.policy_epoch,
        model_epoch: l_id.model_epoch,
        tokenizer_epoch: l_id.tokenizer_epoch,
        codec_epoch: l_id.codec_epoch,
        control_seq: l_id.control_seq.max(r_id.control_seq),
    };
    let l_proofs = lhs.proofs();
    let r_proofs = rhs.proofs();
    let joined_proofs = ProofStates {
        authentic_origin: join_scoped_proof(l_proofs.authentic_origin, r_proofs.authentic_origin),
        complete_for_contract: join_scoped_proof(
            l_proofs.complete_for_contract,
            r_proofs.complete_for_contract,
        ),
        semantically_valid: join_scoped_proof(
            l_proofs.semantically_valid,
            r_proofs.semantically_valid,
        ),
        available_for_replay: join_scoped_proof(
            l_proofs.available_for_replay,
            r_proofs.available_for_replay,
        ),
    };
    Ok(Basis::new(
        joined_identity,
        weaker_claim_class(lhs.claim_class(), rhs.claim_class()),
        joined_proofs,
    ))
}

/// Reason why [`Knowledge::as_known`] refused to unwrap a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KnowledgeAccessError {
    NotKnown {
        variant_name: &'static str,
        would_change_this: String,
    },
    StaleGeneration {
        basis_generation: u64,
        required_generation: u64,
        would_change_this: String,
    },
    InsufficientClaimClass {
        actual: ClaimClass,
        required_min: ClaimClass,
        would_change_this: String,
    },
    UnverifiedProofState {
        would_change_this: String,
    },
}

fn proofs_all_declared(proofs: ProofStates) -> bool {
    matches!(proofs.authentic_origin, ScopedProof::Declared { .. })
        && matches!(proofs.complete_for_contract, ScopedProof::Declared { .. })
        && matches!(proofs.semantically_valid, ScopedProof::Declared { .. })
        && matches!(proofs.available_for_replay, ScopedProof::Declared { .. })
}

impl<T> Knowledge<T> {
    /// Short canonical tag name for diagnostics and structured logs.
    pub const fn variant_name(&self) -> &'static str {
        match self {
            Self::Known { .. } => "Known",
            Self::Pending { .. } => "Pending",
            Self::Unknown { .. } => "Unknown",
            Self::Withheld { .. } => "Withheld",
            Self::Stale { .. } => "Stale",
            Self::Absent { .. } => "Absent",
        }
    }

    /// Transform the payload of a `Known` value while preserving its `Basis` and
    /// leaving all non-`Known` variants unchanged.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Knowledge<U> {
        match self {
            Self::Known { value, basis } => Knowledge::Known {
                value: f(value),
                basis,
            },
            Self::Pending {
                frontiers,
                expected_cost,
            } => Knowledge::Pending {
                frontiers,
                expected_cost,
            },
            Self::Unknown { reason } => Knowledge::Unknown { reason },
            Self::Withheld { authority_required } => Knowledge::Withheld { authority_required },
            Self::Stale {
                basis,
                current_generation,
            } => Knowledge::Stale {
                basis,
                current_generation,
            },
            Self::Absent { domain } => Knowledge::Absent { domain },
        }
    }

    /// Chain a computation that itself returns `Knowledge<U>`, conservatively
    /// joining the outer `Basis` with the inner result so that `and_then` can
    /// never upgrade a weaker `ClaimClass` or incompatible generation.
    pub fn and_then<U, F: FnOnce(T, Basis) -> Knowledge<U>>(self, f: F) -> Knowledge<U> {
        match self {
            Self::Known { value, basis } => match f(value, basis) {
                Knowledge::Known {
                    value: next_val,
                    basis: next_basis,
                } => match join_basis_conservative(basis, next_basis) {
                    Ok(joined_basis) => Knowledge::Known {
                        value: next_val,
                        basis: joined_basis,
                    },
                    Err(reason) => Knowledge::Unknown { reason },
                },
                other => other,
            },
            Self::Pending {
                frontiers,
                expected_cost,
            } => Knowledge::Pending {
                frontiers,
                expected_cost,
            },
            Self::Unknown { reason } => Knowledge::Unknown { reason },
            Self::Withheld { authority_required } => Knowledge::Withheld { authority_required },
            Self::Stale {
                basis,
                current_generation,
            } => Knowledge::Stale {
                basis,
                current_generation,
            },
            Self::Absent { domain } => Knowledge::Absent { domain },
        }
    }

    /// Lattice-safe conservative join of two `Knowledge` values (`b0qg.1`).
    ///
    /// Precedence laws (never upgrades a non-Known variant to `Known`):
    /// 1. `Unknown` dominates everything (`join(Unknown, _) == join(_, Unknown) == Unknown`).
    /// 2. `Withheld` propagates without leaking the other value (`Withheld` dominates `Stale`, `Pending`, `Absent`, `Known`).
    /// 3. `Stale` dominates `Pending`, `Absent`, `Known`.
    /// 4. `Pending` dominates `Absent` and `Known` (merging frontiers when both are `Pending`).
    /// 5. `Absent` dominates `Known`; two `Absent` values with matching closed domains remain `Absent`,
    ///    or degrade to `Unknown { reason: MissingExactDomain }` if their domains differ.
    /// 6. `Known` + `Known` produces `Known` only if their bases are generation/epoch compatible;
    ///    otherwise `Unknown { reason: IncompatibleGeneration }`.
    pub fn join<U>(self, other: Knowledge<U>) -> Knowledge<(T, U)> {
        match (self, other) {
            (Self::Unknown { reason }, _) | (_, Knowledge::Unknown { reason }) => {
                Knowledge::Unknown { reason }
            }
            (
                Self::Withheld {
                    authority_required: a1,
                },
                Knowledge::Withheld {
                    authority_required: a2,
                },
            ) => Knowledge::Withheld {
                authority_required: a1.max(a2),
            },
            (Self::Withheld { authority_required }, _)
            | (_, Knowledge::Withheld { authority_required }) => {
                Knowledge::Withheld { authority_required }
            }
            (
                Self::Stale {
                    basis: b1,
                    current_generation: g1,
                },
                Knowledge::Stale {
                    basis: b2,
                    current_generation: g2,
                },
            ) => {
                let older_basis = if b1.semantic_generation() <= b2.semantic_generation() {
                    b1
                } else {
                    b2
                };
                Knowledge::Stale {
                    basis: older_basis,
                    current_generation: g1.max(g2),
                }
            }
            (
                Self::Stale {
                    basis,
                    current_generation,
                },
                _,
            )
            | (
                _,
                Knowledge::Stale {
                    basis,
                    current_generation,
                },
            ) => Knowledge::Stale {
                basis,
                current_generation,
            },
            (
                Self::Pending {
                    frontiers: mut f1,
                    expected_cost: c1,
                },
                Knowledge::Pending {
                    frontiers: f2,
                    expected_cost: c2,
                },
            ) => {
                for req in f2 {
                    if !f1.contains(&req) {
                        f1.push(req);
                    }
                }
                Knowledge::Pending {
                    frontiers: f1,
                    expected_cost: c1.saturating_add(c2),
                }
            }
            (
                Self::Pending {
                    frontiers,
                    expected_cost,
                },
                _,
            )
            | (
                _,
                Knowledge::Pending {
                    frontiers,
                    expected_cost,
                },
            ) => Knowledge::Pending {
                frontiers,
                expected_cost,
            },
            (Self::Absent { domain: d1 }, Knowledge::Absent { domain: d2 }) => {
                if d1 == d2 {
                    Knowledge::Absent { domain: d1 }
                } else {
                    Knowledge::Unknown {
                        reason: UnknownReason::MissingExactDomain,
                    }
                }
            }
            (Self::Absent { domain }, Knowledge::Known { .. })
            | (Self::Known { .. }, Knowledge::Absent { domain }) => Knowledge::Absent { domain },
            (
                Self::Known {
                    value: v1,
                    basis: b1,
                },
                Knowledge::Known {
                    value: v2,
                    basis: b2,
                },
            ) => match join_basis_conservative(b1, b2) {
                Ok(joined_basis) => Knowledge::Known {
                    value: (v1, v2),
                    basis: joined_basis,
                },
                Err(reason) => Knowledge::Unknown { reason },
            },
        }
    }

    /// Unwrap a `Known` value only when the caller explicitly names the minimum
    /// [`ClaimClass`] and `required_generation` it relies on (`b0qg.1`).
    ///
    /// Never provides an `unwrap_or(true)` escape hatch.
    pub fn as_known(
        &self,
        required_min_class: ClaimClass,
        required_generation: u64,
    ) -> Result<(&T, Basis), KnowledgeAccessError> {
        match self {
            Self::Known { value, basis } => {
                if basis.semantic_generation() != required_generation {
                    return Err(KnowledgeAccessError::StaleGeneration {
                        basis_generation: basis.semantic_generation(),
                        required_generation,
                        would_change_this: format!(
                            "re-evaluate observation at semantic_generation={required_generation} (current basis generation is {})",
                            basis.semantic_generation()
                        ),
                    });
                }
                if claim_class_rank(basis.claim_class()) < claim_class_rank(required_min_class) {
                    return Err(KnowledgeAccessError::InsufficientClaimClass {
                        actual: basis.claim_class(),
                        required_min: required_min_class,
                        would_change_this: format!(
                            "supply evidence with claim_class >= {required_min_class:?} (actual: {:?})",
                            basis.claim_class()
                        ),
                    });
                }
                if !proofs_all_declared(basis.proofs()) {
                    return Err(KnowledgeAccessError::UnverifiedProofState {
                        would_change_this: "supply Declared proof states for authentic_origin, complete_for_contract, semantically_valid, and available_for_replay".to_owned(),
                    });
                }
                Ok((value, *basis))
            }
            other => Err(KnowledgeAccessError::NotKnown {
                variant_name: other.variant_name(),
                would_change_this: other.would_change_this_hint(),
            }),
        }
    }

    /// Actionable remediation hint naming what would turn a non-`Known` variant
    /// into admissible `Known` evidence (`plan §17.3`).
    pub fn would_change_this_hint(&self) -> String {
        match self {
            Self::Known { basis, .. } => format!(
                "already Known at semantic_generation={} with claim_class={:?}",
                basis.semantic_generation(),
                basis.claim_class()
            ),
            Self::Pending {
                frontiers,
                expected_cost,
            } => format!(
                "advance {} pending frontier requirement(s) (expected_cost={expected_cost})",
                frontiers.len()
            ),
            Self::Unknown { reason } => {
                format!("resolve Unknown({reason:?}) by capturing closed-scope evidence")
            }
            Self::Withheld { authority_required } => {
                format!("present capability granting AuthorityKind::{authority_required:?}")
            }
            Self::Stale {
                basis,
                current_generation,
            } => format!(
                "refresh stale evidence from generation {} to current_generation={current_generation}",
                basis.semantic_generation()
            ),
            Self::Absent { domain } => format!(
                "positively observe an object in closed domain {} (revision={}) if presence is required",
                domain.domain_id(),
                domain.revision()
            ),
        }
    }

    /// Construct `Knowledge::Absent` only after verifying that the frontier is
    /// authenticated and closed by a non-zero closing marker (`b0qg.4`).
    pub fn absent_from_closed_frontier(
        domain: DomainProjection,
        revision: u64,
        control_cut: u64,
        semantic_epoch: u64,
        requirement: FrontierRequirement,
        marker: TrustedClosingMarker,
    ) -> Result<Self, Error> {
        if requirement.stage != FrontierStage::Authenticated
            || requirement.closure.is_none()
            || marker.marker_generation == 0
            || requirement.closure != Some(marker.marker_generation)
            || requirement.through != marker.final_sequence
            || requirement.key != domain.projection()
            || marker.key != domain.projection()
        {
            return Err(Error::InvalidInput);
        }
        // Delegate through observation_algebra::await_closed_scope or construct via validated path
        let closed = crate::observation_algebra::construct_closed_domain_for_surface(
            domain,
            revision,
            control_cut,
            semantic_epoch,
            requirement,
            marker,
        )?;
        Ok(Self::Absent { domain: closed })
    }
}

/// Marker trait implemented ONLY by domain types that may cross a layer boundary
/// or be serialized onto an agent-facing surface inside `Knowledge<T>` (`FA-INV-039`, `b0qg.2`).
///
/// Bare primitives (`bool`, `u64`, `String`) deliberately do NOT implement `SafetyRelevant`.
pub trait SafetyRelevant {
    fn safety_type_name() -> &'static str;
    fn canonical_payload_string(&self) -> String;

    /// JSON payload for the known value. Existing implementations retain their
    /// textual representation as an escaped JSON string. Domain types may
    /// override this to return structured JSON; the surface validates it under
    /// bounded strict parsing before embedding it inside the Knowledge wrapper.
    fn canonical_payload_json(&self) -> String {
        quote_json(&self.canonical_payload_string())
    }
}

/// Typed safety verdict (replaces bare `bool` `safe` fields above L0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SafetyVerdict {
    AdmissibleWithinEnvelope { envelope_id: u64 },
    Disqualified { rule_id: u64 },
}

impl SafetyRelevant for SafetyVerdict {
    fn safety_type_name() -> &'static str {
        "SafetyVerdict"
    }

    fn canonical_payload_string(&self) -> String {
        match self {
            Self::AdmissibleWithinEnvelope { envelope_id } => {
                format!("AdmissibleWithinEnvelope({envelope_id})")
            }
            Self::Disqualified { rule_id } => format!("Disqualified({rule_id})"),
        }
    }
}

/// Typed permit eligibility (never a bare `bool`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermitEligibility {
    pub action_digest: u64,
    pub policy_epoch: u64,
    pub max_units: u64,
}

impl SafetyRelevant for PermitEligibility {
    fn safety_type_name() -> &'static str {
        "PermitEligibility"
    }

    fn canonical_payload_string(&self) -> String {
        format!(
            "PermitEligibility(action={},epoch={},units={})",
            self.action_digest, self.policy_epoch, self.max_units
        )
    }
}

/// Serialized representation of an agent-surface safety field (`b0qg.2`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerializedAgentField {
    pub field_name: String,
    pub safety_type: &'static str,
    pub variant: &'static str,
    pub canonical_json: String,
}

/// Bounds for a complete encoded Knowledge field. These are format/resource
/// bounds, not proof of the value's authority or of its observation provenance.
pub const KNOWLEDGE_JSON_LIMITS: crate::strict_json::Limits = crate::strict_json::Limits {
    max_bytes: 1024 * 1024,
    max_depth: 64,
    max_items: 32_768,
    max_string_bytes: 256 * 1024,
};

const MAX_KNOWLEDGE_METADATA_BYTES: usize = 1024;

/// Leave room for the epistemic wrapper and its complete basis around a custom
/// payload. A SafetyRelevant implementation is trusted Rust code; these limits
/// bound parsing and embedding its returned string, not the implementation's work.
const PAYLOAD_JSON_LIMITS: crate::strict_json::Limits = crate::strict_json::Limits {
    max_bytes: KNOWLEDGE_JSON_LIMITS.max_bytes,
    max_depth: 56,
    max_items: 30_000,
    max_string_bytes: KNOWLEDGE_JSON_LIMITS.max_string_bytes,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SerializerRefusal {
    BareScalarRejected {
        field_name: String,
        detected_type: String,
        log_line: String,
    },
    /// Includes malformed syntax, duplicate keys and the strict payload bounds.
    /// No untrusted payload text is copied into this refusal.
    InvalidPayloadJson { safety_type: &'static str },
    EncodingLimitExceeded {
        component: &'static str,
        limit: usize,
    },
}

/// Encode one complete epistemic value without an enclosing field name.
///
/// All six Knowledge variants retain their distinguishing data. Known and Stale
/// include the entire basis, Pending names each source/projection frontier, and
/// Absent retains the exact domain, closed frontier and caller-declared marker.
/// This records reference evidence; it does not authenticate a declared proof.
pub fn encode_knowledge_json<T: SafetyRelevant>(
    wrapped: &Knowledge<T>,
) -> Result<String, SerializerRefusal> {
    bounded_knowledge_json(encode_object(&knowledge_json_fields(wrapped)?))
}

fn bounded_knowledge_json(encoded: String) -> Result<String, SerializerRefusal> {
    if encoded.len() > KNOWLEDGE_JSON_LIMITS.max_bytes {
        return Err(SerializerRefusal::EncodingLimitExceeded {
            component: "encoded_knowledge",
            limit: KNOWLEDGE_JSON_LIMITS.max_bytes,
        });
    }
    Ok(encoded)
}

fn knowledge_json_fields<T: SafetyRelevant>(
    wrapped: &Knowledge<T>,
) -> Result<Vec<(&'static str, String)>, SerializerRefusal> {
    let safety_type = T::safety_type_name();
    if safety_type.len() > MAX_KNOWLEDGE_METADATA_BYTES {
        return Err(SerializerRefusal::EncodingLimitExceeded {
            component: "safety_type",
            limit: MAX_KNOWLEDGE_METADATA_BYTES,
        });
    }
    let mut fields = vec![
        ("type", quote_json(safety_type)),
        ("state", quote_json(wrapped.variant_name())),
    ];
    match wrapped {
        Knowledge::Known { value, basis } => {
            let payload = value.canonical_payload_json();
            let parsed = crate::strict_json::parse(payload.as_bytes(), PAYLOAD_JSON_LIMITS)
                .map_err(|_| SerializerRefusal::InvalidPayloadJson { safety_type })?;
            fields.extend([
                ("value", encode_json_value(&parsed)),
                (
                    "claim_class",
                    quote_json(&format!("{:?}", basis.claim_class())),
                ),
                ("generation", basis.semantic_generation().to_string()),
                ("control_seq", basis.control_seq().to_string()),
                ("basis", encode_basis(*basis)),
            ]);
        }
        Knowledge::Pending {
            frontiers,
            expected_cost,
        } => {
            if frontiers.len() > crate::observation_algebra::MAX_OPERATOR_FRONTIERS {
                return Err(SerializerRefusal::EncodingLimitExceeded {
                    component: "frontiers",
                    limit: crate::observation_algebra::MAX_OPERATOR_FRONTIERS,
                });
            }
            fields.extend([
                ("frontier_count", frontiers.len().to_string()),
                (
                    "frontiers",
                    format!(
                        "[{}]",
                        frontiers
                            .iter()
                            .map(|frontier| encode_frontier(*frontier))
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                ),
                ("expected_cost", expected_cost.to_string()),
                (
                    "would_change_this",
                    quote_json(&wrapped.would_change_this_hint()),
                ),
            ]);
        }
        Knowledge::Unknown { reason } => {
            fields.extend([
                ("reason", quote_json(&format!("{reason:?}"))),
                (
                    "would_change_this",
                    quote_json(&wrapped.would_change_this_hint()),
                ),
            ]);
        }
        Knowledge::Withheld { authority_required } => {
            fields.extend([
                (
                    "authority_required",
                    quote_json(&format!("{authority_required:?}")),
                ),
                (
                    "would_change_this",
                    quote_json(&wrapped.would_change_this_hint()),
                ),
            ]);
        }
        Knowledge::Stale {
            basis,
            current_generation,
        } => {
            fields.extend([
                ("basis_generation", basis.semantic_generation().to_string()),
                ("current_generation", current_generation.to_string()),
                ("basis", encode_basis(*basis)),
                (
                    "would_change_this",
                    quote_json(&wrapped.would_change_this_hint()),
                ),
            ]);
        }
        Knowledge::Absent { domain } => {
            fields.extend([
                ("closed_domain_id", domain.domain_id().to_string()),
                ("revision", domain.revision().to_string()),
                ("domain", encode_closed_domain(domain)),
                (
                    "would_change_this",
                    quote_json(&wrapped.would_change_this_hint()),
                ),
            ]);
        }
    }
    Ok(fields)
}

/// JSON string quoting is required even for field/type names: a caller-defined
/// SafetyRelevant type is not an exemption from the wire syntax boundary.
fn quote_json(value: &str) -> String {
    let mut encoded = String::new();
    encoded.push('"');
    for ch in value.chars() {
        match ch {
            '"' => encoded.push_str("\\\""),
            '\\' => encoded.push_str("\\\\"),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\t' => encoded.push_str("\\t"),
            '\u{08}' => encoded.push_str("\\b"),
            '\u{0c}' => encoded.push_str("\\f"),
            ch if ch <= '\u{1f}' => {
                let _ = write!(encoded, "\\u{:04x}", ch as u32);
            }
            ch => encoded.push(ch),
        }
    }
    encoded.push('"');
    encoded
}

fn encode_object(fields: &[(&str, String)]) -> String {
    format!(
        "{{{}}}",
        fields
            .iter()
            .map(|(name, value)| format!("{}:{value}", quote_json(name)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Parsing rejects duplicate keys before this deterministic re-encoding. Object
/// order and whitespace cannot change the emitted structure, and exact JSON
/// number spelling is retained without a lossy floating-point conversion.
fn encode_json_value(value: &crate::strict_json::Json) -> String {
    use crate::strict_json::Json;
    match value {
        Json::Null => "null".to_owned(),
        Json::Bool(value) => value.to_string(),
        Json::Number(value) => value.lexeme().to_owned(),
        Json::String(value) => quote_json(value),
        Json::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(encode_json_value)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Json::Object(fields) => format!(
            "{{{}}}",
            fields
                .iter()
                .map(|(name, value)| format!("{}:{}", quote_json(name), encode_json_value(value)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn encode_basis(basis: Basis) -> String {
    let identity = basis.identity();
    let proofs = basis.proofs();
    encode_object(&[
        (
            "identity",
            encode_object(&[
                ("subject_id", identity.subject_id.to_string()),
                ("question_id", identity.question_id.to_string()),
                ("profile_id", identity.profile_id.to_string()),
                (
                    "semantic_generation",
                    identity.semantic_generation.to_string(),
                ),
                ("policy_epoch", identity.policy_epoch.to_string()),
                ("model_epoch", identity.model_epoch.to_string()),
                ("tokenizer_epoch", identity.tokenizer_epoch.to_string()),
                ("codec_epoch", identity.codec_epoch.to_string()),
                ("control_seq", identity.control_seq.to_string()),
            ]),
        ),
        (
            "claim_class",
            quote_json(&format!("{:?}", basis.claim_class())),
        ),
        (
            "proofs",
            encode_object(&[
                (
                    "authentic_origin",
                    encode_scoped_proof(proofs.authentic_origin),
                ),
                (
                    "complete_for_contract",
                    encode_scoped_proof(proofs.complete_for_contract),
                ),
                (
                    "semantically_valid",
                    encode_scoped_proof(proofs.semantically_valid),
                ),
                (
                    "available_for_replay",
                    encode_scoped_proof(proofs.available_for_replay),
                ),
            ]),
        ),
    ])
}

fn encode_scoped_proof(proof: ScopedProof) -> String {
    match proof {
        ScopedProof::Declared {
            scope_id,
            generation,
        } => encode_object(&[
            ("state", quote_json("Declared")),
            ("scope_id", scope_id.to_string()),
            ("generation", generation.to_string()),
        ]),
        ScopedProof::Gapped {
            scope_id,
            first_missing,
        } => encode_object(&[
            ("state", quote_json("Gapped")),
            ("scope_id", scope_id.to_string()),
            ("first_missing", first_missing.to_string()),
        ]),
        ScopedProof::Unsupported { scope_id } => encode_object(&[
            ("state", quote_json("Unsupported")),
            ("scope_id", scope_id.to_string()),
        ]),
        ScopedProof::Expired {
            scope_id,
            generation,
        } => encode_object(&[
            ("state", quote_json("Expired")),
            ("scope_id", scope_id.to_string()),
            ("generation", generation.to_string()),
        ]),
    }
}

fn encode_projection(key: crate::product_frontier::ProjectionKey) -> String {
    encode_object(&[
        ("source", key.source.to_string()),
        ("branch", key.branch.to_string()),
        ("projection", key.projection.to_string()),
        ("source_epoch", key.source_epoch.to_string()),
    ])
}

fn encode_frontier(frontier: FrontierRequirement) -> String {
    encode_object(&[
        ("key", encode_projection(frontier.key)),
        ("stage", quote_json(&format!("{:?}", frontier.stage))),
        ("through", frontier.through.to_string()),
        (
            "closure",
            frontier
                .closure
                .map_or_else(|| "null".to_owned(), |value| value.to_string()),
        ),
    ])
}

fn encode_closed_domain(domain: &ClosedDomain) -> String {
    let projection = domain.domain();
    let marker = domain.marker();
    encode_object(&[
        ("domain_id", projection.domain_id().to_string()),
        ("domain_epoch", projection.domain_epoch().to_string()),
        ("projection", encode_projection(projection.projection())),
        ("revision", domain.revision().to_string()),
        ("control_cut", domain.control_cut().to_string()),
        ("semantic_epoch", domain.semantic_epoch().to_string()),
        ("requirement", encode_frontier(domain.requirement())),
        (
            "marker",
            encode_object(&[
                ("key", encode_projection(marker.key)),
                ("final_sequence", marker.final_sequence.to_string()),
                ("marker_generation", marker.marker_generation.to_string()),
            ]),
        ),
    ])
}

pub struct AgentSurfaceSerializer;

impl AgentSurfaceSerializer {
    /// Compatibility entry point for existing domain surfaces. Invalid custom
    /// payloads become an explicit Unknown integrity refusal, never malformed
    /// JSON or a Known value. New callers should use the fallible method below.
    pub fn serialize_safety_field<T: SafetyRelevant>(
        field_name: &str,
        wrapped: &Knowledge<T>,
    ) -> SerializedAgentField {
        match Self::try_serialize_safety_field(field_name, wrapped) {
            Ok(field) => field,
            Err(_) => {
                let bounded_metadata = |value: &str| {
                    if value.len() <= MAX_KNOWLEDGE_METADATA_BYTES {
                        quote_json(value)
                    } else {
                        quote_json("<metadata-limit-exceeded>")
                    }
                };
                SerializedAgentField {
                    field_name: field_name.to_owned(),
                    safety_type: T::safety_type_name(),
                    variant: "Unknown",
                    canonical_json: encode_object(&[
                        ("field", bounded_metadata(field_name)),
                        ("type", bounded_metadata(T::safety_type_name())),
                        ("state", quote_json("Unknown")),
                        ("reason", quote_json("ConflictingEvidence")),
                        ("serialization_error", quote_json("IntegrityFailure")),
                        (
                            "would_change_this",
                            quote_json("supply valid bounded Knowledge JSON"),
                        ),
                    ]),
                }
            }
        }
    }

    /// Compile-time and runtime enforced serializer accepting only Knowledge of
    /// a SafetyRelevant domain type. Withheld output depends on the requested
    /// field/type and required authority, never on hidden object existence.
    pub fn try_serialize_safety_field<T: SafetyRelevant>(
        field_name: &str,
        wrapped: &Knowledge<T>,
    ) -> Result<SerializedAgentField, SerializerRefusal> {
        if field_name.len() > MAX_KNOWLEDGE_METADATA_BYTES {
            return Err(SerializerRefusal::EncodingLimitExceeded {
                component: "field_name",
                limit: MAX_KNOWLEDGE_METADATA_BYTES,
            });
        }
        let mut fields = knowledge_json_fields(wrapped)?;
        fields.insert(0, ("field", quote_json(field_name)));
        Ok(SerializedAgentField {
            field_name: field_name.to_owned(),
            safety_type: T::safety_type_name(),
            variant: wrapped.variant_name(),
            canonical_json: bounded_knowledge_json(encode_object(&fields))?,
        })
    }

    /// Check bounded strict JSON syntax and a real, recognized top-level
    /// Knowledge wrapper, rather than accepting the word "state" inside prose.
    /// This is a shape guard, not an evidence decoder or permit-admission gate.
    pub fn validate_wire_field_is_wrapped(
        field_name: &str,
        raw_value_fragment: &str,
    ) -> Result<(), SerializerRefusal> {
        use crate::strict_json::Json;
        let parsed =
            crate::strict_json::parse(raw_value_fragment.as_bytes(), KNOWLEDGE_JSON_LIMITS);
        let detected_type = match parsed {
            Ok(Json::Bool(_)) => Some("bool"),
            Ok(Json::String(_)) => Some("bare_string"),
            Ok(Json::Number(number)) => Some(if number.as_u64().is_some() {
                "bare_integer"
            } else {
                "bare_number"
            }),
            Ok(Json::Object(fields))
                if fields.get("type").and_then(Json::as_str).is_some()
                    && matches!(
                        fields.get("state").and_then(Json::as_str),
                        Some("Known" | "Pending" | "Unknown" | "Withheld" | "Stale" | "Absent")
                    ) =>
            {
                None
            }
            _ => Some("unwrapped_object"),
        };

        if let Some(bad_type) = detected_type {
            let log_line = format!(
                "event=serializer_refusal field={} rejected_type={bad_type} reason=FA-INV-039_bare_safety_scalar_forbidden",
                quote_json(field_name),
            );
            return Err(SerializerRefusal::BareScalarRejected {
                field_name: field_name.to_owned(),
                detected_type: bad_type.to_owned(),
                log_line,
            });
        }
        Ok(())
    }
}

/// Policy governing what `Knowledge<T>` basis is required to admit a permit (`b0qg.3`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermitKnowledgePolicy {
    pub min_claim_class: ClaimClass,
    pub required_generation: u64,
    pub required_policy_epoch: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermitEvidenceAdmission<'a, T> {
    pub value: &'a T,
    pub basis: Basis,
    pub decision_log: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PermitEvidenceRefusal {
    MissingRequiredObservation {
        blocking_variant: &'static str,
        would_change_this: String,
        decision_log: String,
    },
    DisallowedClaimClass {
        actual: ClaimClass,
        required_min: ClaimClass,
        would_change_this: String,
        decision_log: String,
    },
    StaleOrMismatchedBasis {
        basis_generation: u64,
        required_generation: u64,
        basis_policy_epoch: u64,
        required_policy_epoch: u64,
        would_change_this: String,
        decision_log: String,
    },
    IncompleteProofs {
        would_change_this: String,
        decision_log: String,
    },
}

/// Dispatch predicate that consumes `Knowledge<T>` and admits ONLY on `Known`
/// with a policy-allowed `ClaimClass`, matching generation/epoch, and declared proofs.
///
/// Every non-`Known` variant (`Unknown`, `Pending`, `Withheld`, `Stale`, `Absent`)
/// yields [`PermitEvidenceRefusal::MissingRequiredObservation`] with `would_change_this`
/// naming the exact frontier, authority, or generation required (`FA-INV-004`, `b0qg.3`).
pub fn evaluate_knowledge_for_permit<'a, T: SafetyRelevant>(
    evidence: &'a Knowledge<T>,
    policy: &PermitKnowledgePolicy,
) -> Result<PermitEvidenceAdmission<'a, T>, PermitEvidenceRefusal> {
    let type_name = T::safety_type_name();
    match evidence {
        Knowledge::Known { value, basis } => {
            if basis.semantic_generation() != policy.required_generation
                || basis.policy_epoch() != policy.required_policy_epoch
            {
                let would_change_this = format!(
                    "re-capture {type_name} at semantic_generation={} and policy_epoch={} (got generation={}, epoch={})",
                    policy.required_generation,
                    policy.required_policy_epoch,
                    basis.semantic_generation(),
                    basis.policy_epoch()
                );
                let decision_log = format!(
                    "event=permit_evidence_decision type={type_name} outcome=deny blocking_variant=KnownStaleBasis basis_gen={} required_gen={} would_change_this=\"{would_change_this}\"",
                    basis.semantic_generation(),
                    policy.required_generation
                );
                return Err(PermitEvidenceRefusal::StaleOrMismatchedBasis {
                    basis_generation: basis.semantic_generation(),
                    required_generation: policy.required_generation,
                    basis_policy_epoch: basis.policy_epoch(),
                    required_policy_epoch: policy.required_policy_epoch,
                    would_change_this,
                    decision_log,
                });
            }
            if claim_class_rank(basis.claim_class()) < claim_class_rank(policy.min_claim_class) {
                let would_change_this = format!(
                    "upgrade {type_name} evidence from {:?} to at least {:?}",
                    basis.claim_class(),
                    policy.min_claim_class
                );
                let decision_log = format!(
                    "event=permit_evidence_decision type={type_name} outcome=deny blocking_variant=KnownWeakClaimClass actual={:?} required_min={:?}",
                    basis.claim_class(),
                    policy.min_claim_class
                );
                return Err(PermitEvidenceRefusal::DisallowedClaimClass {
                    actual: basis.claim_class(),
                    required_min: policy.min_claim_class,
                    would_change_this,
                    decision_log,
                });
            }
            if !proofs_all_declared(basis.proofs()) {
                let would_change_this = format!(
                    "complete all four ProofStates for {type_name} at generation={}",
                    policy.required_generation
                );
                let decision_log = format!(
                    "event=permit_evidence_decision type={type_name} outcome=deny blocking_variant=KnownIncompleteProofs"
                );
                return Err(PermitEvidenceRefusal::IncompleteProofs {
                    would_change_this,
                    decision_log,
                });
            }
            let decision_log = format!(
                "event=permit_evidence_decision type={type_name} outcome=admit variant=Known claim_class={:?} generation={}",
                basis.claim_class(),
                basis.semantic_generation()
            );
            Ok(PermitEvidenceAdmission {
                value,
                basis: *basis,
                decision_log,
            })
        }
        non_known => {
            let blocking_variant = non_known.variant_name();
            let would_change_this = non_known.would_change_this_hint();
            let decision_log = format!(
                "event=permit_evidence_decision type={type_name} outcome=deny reason=MissingRequiredObservation blocking_variant={blocking_variant} would_change_this=\"{would_change_this}\""
            );
            Err(PermitEvidenceRefusal::MissingRequiredObservation {
                blocking_variant,
                would_change_this,
                decision_log,
            })
        }
    }
}

/// Deterministic golden byte encoding for `Knowledge<SafetyVerdict>` (`b0qg.1`).
pub fn encode_knowledge_verdict_v1(k: &Knowledge<SafetyVerdict>) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(KNOWLEDGE_ENCODING_V1);
    match k {
        Knowledge::Known { value, basis } => {
            out.push(0x01);
            match value {
                SafetyVerdict::AdmissibleWithinEnvelope { envelope_id } => {
                    out.push(0x01);
                    out.extend_from_slice(&envelope_id.to_be_bytes());
                }
                SafetyVerdict::Disqualified { rule_id } => {
                    out.push(0x02);
                    out.extend_from_slice(&rule_id.to_be_bytes());
                }
            }
            out.push(claim_class_rank(basis.claim_class()));
            out.extend_from_slice(&basis.semantic_generation().to_be_bytes());
            out.extend_from_slice(&basis.policy_epoch().to_be_bytes());
            out.extend_from_slice(&basis.control_seq().to_be_bytes());
        }
        Knowledge::Pending {
            frontiers,
            expected_cost,
        } => {
            out.push(0x02);
            out.extend_from_slice(&(frontiers.len() as u32).to_be_bytes());
            out.extend_from_slice(&expected_cost.to_be_bytes());
        }
        Knowledge::Unknown { reason } => {
            out.push(0x03);
            let reason_tag = match reason {
                UnknownReason::Cancelled => 1,
                UnknownReason::Incomplete => 2,
                UnknownReason::IncompatibleGeneration => 3,
                UnknownReason::MissingExactDomain => 4,
                UnknownReason::LowerLayerUnverified => 5,
                UnknownReason::ConflictingEvidence => 6,
                UnknownReason::Unavailable => 7,
            };
            out.push(reason_tag);
        }
        Knowledge::Withheld { authority_required } => {
            out.push(0x04);
            let auth_tag = match authority_required {
                AuthorityKind::Observe => 1,
                AuthorityKind::Project => 2,
                AuthorityKind::Verify => 3,
                AuthorityKind::ExactRead => 4,
                AuthorityKind::Discover => 5,
                AuthorityKind::Probe => 6,
                AuthorityKind::Judge => 7,
                AuthorityKind::CloseScope => 8,
            };
            out.push(auth_tag);
        }
        Knowledge::Stale {
            basis,
            current_generation,
        } => {
            out.push(0x05);
            out.extend_from_slice(&basis.semantic_generation().to_be_bytes());
            out.extend_from_slice(&current_generation.to_be_bytes());
        }
        Knowledge::Absent { domain } => {
            out.push(0x06);
            out.extend_from_slice(&domain.domain_id().to_be_bytes());
            out.extend_from_slice(&domain.revision().to_be_bytes());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        product_frontier::{
            FrontierRequirement, FrontierStage, ProjectionKey, TrustedClosingMarker,
        },
        witness::DomainProjection,
    };

    fn sample_basis(generation: u64, policy_epoch: u64, class: ClaimClass) -> Basis {
        let declared = ScopedProof::Declared {
            scope_id: 1,
            generation,
        };
        Basis::new(
            BasisIdentity {
                subject_id: 10,
                question_id: 20,
                profile_id: 30,
                semantic_generation: generation,
                policy_epoch,
                model_epoch: 1,
                tokenizer_epoch: 1,
                codec_epoch: 1,
                control_seq: 100,
            },
            class,
            ProofStates {
                authentic_origin: declared,
                complete_for_contract: declared,
                semantically_valid: declared,
                available_for_replay: declared,
            },
        )
    }

    fn sample_closed_domain(domain_id: u64) -> ClosedDomain {
        let key = ProjectionKey {
            source: domain_id,
            branch: 1,
            projection: 1,
            source_epoch: 1,
        };
        let domain = DomainProjection::new(domain_id, 1, key);
        let req = FrontierRequirement {
            key,
            stage: FrontierStage::Authenticated,
            through: 50,
            closure: Some(7),
        };
        let marker = TrustedClosingMarker {
            key,
            final_sequence: 50,
            marker_generation: 7,
        };
        crate::observation_algebra::construct_closed_domain_for_surface(
            domain, 1, 10, 1, req, marker,
        )
        .expect("closed domain should construct")
    }

    #[test]
    fn knowledge_join_lattice_laws_and_no_upgrade_from_non_known() {
        let b_proof = sample_basis(5, 2, ClaimClass::Proof);
        let b_stat = sample_basis(5, 2, ClaimClass::Statistical);
        let key = ProjectionKey {
            source: 1,
            branch: 1,
            projection: 1,
            source_epoch: 1,
        };
        let req = FrontierRequirement {
            key,
            stage: FrontierStage::Authenticated,
            through: 10,
            closure: Some(1),
        };
        let closed = sample_closed_domain(1);

        let variants: Vec<Knowledge<SafetyVerdict>> = vec![
            Knowledge::known(
                SafetyVerdict::AdmissibleWithinEnvelope { envelope_id: 1 },
                b_proof,
            ),
            Knowledge::pending(vec![req], 15).expect("valid pending"),
            Knowledge::unknown(UnknownReason::Incomplete),
            Knowledge::withheld(AuthorityKind::ExactRead),
            Knowledge::stale(b_stat, 6),
            Knowledge::absent(closed),
        ];

        // Exhaustive 6x6 pairwise check:
        // 1. Joining with Unknown ALWAYS yields Unknown.
        // 2. Joining any non-Known variant with Known NEVER upgrades to Known.
        for (i, lhs) in variants.iter().enumerate() {
            for (j, rhs) in variants.iter().enumerate() {
                let joined = lhs.clone().join(rhs.clone());
                if matches!(lhs, Knowledge::Unknown { .. })
                    || matches!(rhs, Knowledge::Unknown { .. })
                {
                    assert!(
                        matches!(joined, Knowledge::Unknown { .. }),
                        "pair ({i}, {j}) must be Unknown when either input is Unknown"
                    );
                }
                if !matches!(lhs, Knowledge::Known { .. })
                    || !matches!(rhs, Knowledge::Known { .. })
                {
                    assert!(
                        !matches!(joined, Knowledge::Known { .. }),
                        "pair ({i}, {j}) upgraded non-Known to Known!"
                    );
                }
            }
        }

        // Joining two compatible Known values picks the weaker ClaimClass.
        let k_proof = Knowledge::known(
            SafetyVerdict::AdmissibleWithinEnvelope { envelope_id: 1 },
            b_proof,
        );
        let k_stat = Knowledge::known(
            SafetyVerdict::AdmissibleWithinEnvelope { envelope_id: 2 },
            b_stat,
        );
        let joined_known = k_proof.join(k_stat);
        let (_, joined_basis) = joined_known
            .as_known(ClaimClass::Statistical, 5)
            .expect("compatible Known join should succeed at Statistical rank");
        assert_eq!(joined_basis.claim_class(), ClaimClass::Statistical);
        assert!(
            joined_known.as_known(ClaimClass::Proof, 5).is_err(),
            "joined Known must not claim stronger Proof class when one input was Statistical"
        );
    }

    #[test]
    fn serializer_accepts_wrapped_safety_types_and_rejects_bare_wire_scalars() {
        let b = sample_basis(3, 1, ClaimClass::Invariant);
        let wrapped = Knowledge::known(
            SafetyVerdict::AdmissibleWithinEnvelope { envelope_id: 42 },
            b,
        );
        let serialized = AgentSurfaceSerializer::serialize_safety_field("verdict", &wrapped);
        assert_eq!(serialized.variant, "Known");
        assert_eq!(serialized.safety_type, "SafetyVerdict");
        assert!(
            AgentSurfaceSerializer::validate_wire_field_is_wrapped(
                "verdict",
                &serialized.canonical_json
            )
            .is_ok()
        );

        for (field, raw, expected_type) in [
            ("safe", "true", "bool"),
            ("permitted", "false", "bool"),
            ("clearance", "7", "bare_integer"),
            ("status", "\"ok\"", "bare_string"),
        ] {
            let err = AgentSurfaceSerializer::validate_wire_field_is_wrapped(field, raw)
                .expect_err("bare scalar must be refused");
            match err {
                SerializerRefusal::BareScalarRejected {
                    field_name,
                    detected_type,
                    log_line,
                } => {
                    assert_eq!(field_name, field);
                    assert_eq!(detected_type, expected_type);
                    assert!(log_line.contains("FA-INV-039_bare_safety_scalar_forbidden"));
                }
                other => panic!("expected bare scalar refusal, got {other:?}"),
            }
        }
    }

    #[test]
    fn permit_path_admits_only_known_and_rejects_all_non_known_variants() {
        let policy = PermitKnowledgePolicy {
            min_claim_class: ClaimClass::BoundedModel,
            required_generation: 10,
            required_policy_epoch: 4,
        };
        let valid_basis = sample_basis(10, 4, ClaimClass::Proof);
        let known_ok = Knowledge::known(
            PermitEligibility {
                action_digest: 99,
                policy_epoch: 4,
                max_units: 5,
            },
            valid_basis,
        );
        let admitted = evaluate_knowledge_for_permit(&known_ok, &policy)
            .expect("valid Known evidence should admit");
        assert_eq!(admitted.value.action_digest, 99);
        assert!(admitted.decision_log.contains("outcome=admit"));

        let key = ProjectionKey {
            source: 2,
            branch: 1,
            projection: 1,
            source_epoch: 1,
        };
        let req = FrontierRequirement {
            key,
            stage: FrontierStage::Authenticated,
            through: 10,
            closure: Some(1),
        };
        let non_known_cases: Vec<(&str, Knowledge<PermitEligibility>)> = vec![
            (
                "Pending",
                Knowledge::pending(vec![req], 25).expect("valid pending"),
            ),
            ("Unknown", Knowledge::unknown(UnknownReason::Incomplete)),
            ("Withheld", Knowledge::withheld(AuthorityKind::Verify)),
            ("Stale", Knowledge::stale(valid_basis, 11)),
            ("Absent", Knowledge::absent(sample_closed_domain(2))),
        ];

        for (expected_name, candidate) in &non_known_cases {
            let err = evaluate_knowledge_for_permit(candidate, &policy)
                .expect_err("non-Known variant must never admit permit");
            match err {
                PermitEvidenceRefusal::MissingRequiredObservation {
                    blocking_variant,
                    would_change_this,
                    decision_log,
                } => {
                    assert_eq!(blocking_variant, *expected_name);
                    assert!(!would_change_this.is_empty());
                    assert!(decision_log.contains("reason=MissingRequiredObservation"));
                }
                other => panic!("expected MissingRequiredObservation, got {other:?}"),
            }
        }
    }

    #[test]
    fn negative_campaign_open_frontier_absent_withheld_leakage_and_stale_reuse() {
        // 1. Absent constructed over an open frontier is refused.
        let key = ProjectionKey {
            source: 9,
            branch: 1,
            projection: 1,
            source_epoch: 1,
        };
        let domain = DomainProjection::new(9, 1, key);
        let open_req = FrontierRequirement {
            key,
            stage: FrontierStage::Captured,
            through: 50,
            closure: None,
        };
        let unclosed_marker = TrustedClosingMarker {
            key,
            final_sequence: 50,
            marker_generation: 0,
        };
        assert_eq!(
            Knowledge::<SafetyVerdict>::absent_from_closed_frontier(
                domain,
                1,
                10,
                1,
                open_req,
                unclosed_marker,
            ),
            Err(Error::InvalidInput),
            "Absent over open frontier must fail closed"
        );

        // 2. Withheld serialized/error output is byte-for-byte identical whether or not object exists.
        let withheld_when_object_exists: Knowledge<SafetyVerdict> =
            Knowledge::withheld(AuthorityKind::ExactRead);
        let withheld_when_object_missing: Knowledge<SafetyVerdict> =
            Knowledge::withheld(AuthorityKind::ExactRead);
        let ser_exists =
            AgentSurfaceSerializer::serialize_safety_field("finding", &withheld_when_object_exists);
        let ser_missing = AgentSurfaceSerializer::serialize_safety_field(
            "finding",
            &withheld_when_object_missing,
        );
        assert_eq!(ser_exists.canonical_json, ser_missing.canonical_json);
        assert_eq!(
            encode_knowledge_verdict_v1(&withheld_when_object_exists),
            encode_knowledge_verdict_v1(&withheld_when_object_missing)
        );

        // 3. Stale value or Known value from an older generation used at a newer generation is refused.
        let old_basis = sample_basis(4, 1, ClaimClass::Proof);
        let stale_val: Knowledge<SafetyVerdict> = Knowledge::stale(old_basis, 5);
        assert!(stale_val.as_known(ClaimClass::Proof, 5).is_err());

        let old_known: Knowledge<SafetyVerdict> = Knowledge::known(
            SafetyVerdict::AdmissibleWithinEnvelope { envelope_id: 7 },
            old_basis,
        );
        match old_known.as_known(ClaimClass::Proof, 5) {
            Err(KnowledgeAccessError::StaleGeneration {
                basis_generation: 4,
                required_generation: 5,
                ..
            }) => {}
            other => panic!("expected StaleGeneration refusal, got {other:?}"),
        }
    }
}
