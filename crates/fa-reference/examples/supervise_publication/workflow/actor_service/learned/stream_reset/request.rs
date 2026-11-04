//! Closed independent expected-intent data. No defaults, inferred identifiers,
//! floating-point counters, imported cache or writable state are accepted.
use super::{
    REQUEST_BYTES, Request, debug,
    fields::{self, Fields},
};
use fa_reference::action::consequence::{
    activation::{
        monitor::learned::{LearnedMonitorBudget, model::LearnedAuditPreparationBudget},
        probe::learned::CheckedKvBudget,
        tensor::kv::{
            decoder::monitoring::{LearnedDecoderAllowance, restart::KvRestartBudget},
            model::learned::CompressionBudget,
        },
    },
    delivery::persistent::observed::{
        containment::FileResetRequest, decoder::learned::checkpoint::FileLearnedResetIntent,
    },
    gate::ReviewBinding,
};
use fa_reference::action::{MAX_ATTEMPTS, ResolvedTarget};
use fa_reference::strict_json::{self, Limits};

fn number(fields: &mut Fields, name: &str) -> Result<u64, String> {
    let text = fields.text(name)?;
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!(
            "{name} must be a canonical unsigned decimal string"
        ));
    }
    text.parse::<u64>()
        .map_err(|_| format!("{name} exceeds the unsigned 64-bit range"))
}

fn size(fields: &mut Fields, name: &str) -> Result<usize, String> {
    usize::try_from(number(fields, name)?).map_err(debug)
}

pub(super) fn decode(bytes: &[u8]) -> Result<Request, String> {
    let json = strict_json::parse(
        bytes,
        Limits {
            max_bytes: REQUEST_BYTES,
            max_depth: 8,
            max_items: 16_384,
            max_string_bytes: 64,
        },
    )
    .map_err(debug)?;
    let mut root = Fields::new(json)?;
    if root.text("schema")? != "fa.learned-stream-reset-recovery/1" {
        return Err("unsupported stream-reset recovery schema".into());
    }
    let checkpoint = number(&mut root, "checkpoint")?;
    let first_incident_sequence = number(&mut root, "first_incident_sequence")?;
    if first_incident_sequence == 0 {
        return Err("first incident sequence must be nonzero".into());
    }

    let mut control = root.object("control")?;
    let operation = number(&mut control, "operation")?;
    let expected_control_sequence = number(&mut control, "expected_control_sequence")?;
    let expected_actor_revision = number(&mut control, "expected_actor_revision")?;
    let expected_authority_epoch = number(&mut control, "expected_authority_epoch")?;
    let mut binding = control.object("binding")?;
    let round = number(&mut binding, "round")?;
    let reducer_generation = number(&mut binding, "reducer_generation")?;
    let evidence_root: [u8; 32] = fields::hex(&binding.text("evidence_root_hex")?, 32)?
        .try_into()
        .map_err(|_| "reset evidence root must contain exactly 32 bytes")?;
    binding.end()?;
    let retained_targets = control
        .array("retained_targets", MAX_ATTEMPTS)?
        .into_iter()
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
        })
        .collect::<Result<Vec<_>, _>>()?;
    control.end()?;
    let control = FileResetRequest {
        operation,
        expected_control_sequence,
        expected_actor_revision,
        expected_authority_epoch,
        binding: ReviewBinding {
            round,
            reducer_generation,
            evidence_root,
        },
        retained_targets,
    };

    let mut budget = root.object("restart_budget")?;
    let cache_values = size(&mut budget, "cache_values")?;
    let mut audit = budget.object("audit")?;
    let mut preparation = audit.object("preparation")?;
    let mut compression = preparation.object("compression")?;
    let compression_budget = CompressionBudget {
        source_values: size(&mut compression, "source_values")?,
        encoded_bytes: size(&mut compression, "encoded_bytes")?,
        work_units: number(&mut compression, "work_units")?,
    };
    compression.end()?;
    let mut source_check = preparation.object("source_check")?;
    let source_budget = CheckedKvBudget {
        source_values: size(&mut source_check, "source_values")?,
        encoded_bytes: size(&mut source_check, "encoded_bytes")?,
        reconstruction_products: number(&mut source_check, "reconstruction_products")?,
    };
    source_check.end()?;
    preparation.end()?;
    let mut monitoring = audit.object("monitoring")?;
    let monitoring_budget = LearnedMonitorBudget {
        encoded_bytes: size(&mut monitoring, "encoded_bytes")?,
        probe_coordinates: size(&mut monitoring, "probe_coordinates")?,
        reconstruction_products: number(&mut monitoring, "reconstruction_products")?,
        materialized_values: size(&mut monitoring, "materialized_values")?,
        refinements: size(&mut monitoring, "refinements")?,
    };
    monitoring.end()?;
    audit.end()?;
    budget.end()?;
    root.end()?;
    let budget = KvRestartBudget {
        cache_values,
        audit: LearnedDecoderAllowance {
            preparation: LearnedAuditPreparationBudget {
                compression: compression_budget,
                source_check: source_budget,
            },
            monitoring: monitoring_budget,
        },
    };
    let intent =
        FileLearnedResetIntent::for_recovery(checkpoint, control, budget).map_err(debug)?;
    Ok(Request {
        intent,
        first_incident_sequence,
    })
}
