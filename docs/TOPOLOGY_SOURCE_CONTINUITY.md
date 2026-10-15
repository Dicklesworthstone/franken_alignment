# Preserve source-work continuity through topology recovery

Native guarded recovery deliberately withdraws topology coverage and fences old
roles. Native `bind_withdrawn_topology_source` admits a new custodian with a
recorded operation floor and new-custodian work counters. To instead carry forward
independently retained expenditure, use
`FileMediationObserver::resume_topology_source` with the newly recovered observer
and independently retained `TopologySourceRecovery` requirements instead of
resetting source work or treating the old cut as current.

Before losing the old owner, `source.recovery_requirements(next)` captures the
fixed input paths, exact previous loaded contract, independently chosen newer
contract, spent source work and last operation. Retain the latest requirements
independently. The value is evidence/configuration, not another observer, a
serialized checkpoint, proof of freshness or a durable write. A supervisor may
also assemble it from its independent retained records. Stale/jointly rolled-back
records cannot prove the most recent source expenditure.

Resume delegates to the original withdrawn-source admission, including the
maximum operation ID in acknowledged history. It then validates retained work
and uses the greater of the recorded and retained operation floors. It checks
work arithmetic and remaining allowance before the original native rebind. The
rebind requires both graph and inventory generations to advance and refuses
changed source limits or checker allowance. It acknowledges the original topology
update before installing the next expectation. It does not read source files,
certify a cut, refund effects or reset work. Admission remains blocked until a
fresh original `refresh` verifies both actual files and the native independent
checker accepts the cut. Missing/substituted input takes the original durable
withdrawal path; uncertain storage failures keep the original quarantine.

Install the returned source in `FileDriverTopology` to retain automatic final-
publication checks. A rebind-only result is not a successful source observation.
Operation IDs and source work continue across the handoff. Their independent
retention is the caller's obligation; this API does not add a durable mandatory-
source flag, a background watcher, OS containment, automatic recovery of effect
keys, or authentication of source ownership.

Five native regression functions cover two successive real guarded recoveries,
exact work/operation continuity, uncertified reinstatement, fresh file checks,
source loss/substitution, eleven invalid profile/budget/path/operation cases,
foreign/stale custody, and rejection of live-owner allowance resets. The targeted
remote-only RCH launch was attempted on 2026-10-15 and stopped before compilation
because `rch` is absent (exit 127). Tests are authored, not executed; compilation,
formatting, Clippy and runtime qualification are not claimed.
