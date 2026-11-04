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


## Supervised-driver reconnection

`FileSupervisedDriver::resume_topology` accepts the separate recovered observer
and retained requirements while the original driver has no job or prior topology
selection. It requires remaining allowance and operation IDs for an initial and
final-publication check before changing the native graph. It delegates continuity
to `resume_topology_source`, installs that same custodian, and only then performs
the original actual-file refresh. There is no interval after source I/O in which
success, refusal, failure or unwinding can leave an unselected driver.

The caller must provide current native clock state just as for `refresh_topology`;
this method does not invent time. A returned refusal is not certification. A
source read/check failure retains the original driver diagnostics and custody;
native withdrawal/quarantine still applies. The rebind can have been acknowledged
before that failure. Final publication continues through the original guarded
path and its post-read clock checks. This method does not restore an old job,
issue either effect key, dispatch an effect or prove mandatory source selection
for independently reconstructed drivers that never selected this API.

The recovery fixtures pin the original exact history and terminal reserve before
bounded replay; they do not use a weaker recovery profile. Five additional native
driver regressions cover automatic fresh validation,
retained custody after initial refusal, the subsequent loss path, two-check and
operation-space admission, larger recorded operation floors, and retained
unpersisted operation floors. The multi-recovery fixture uses a high incident
threshold so its intended repeated fencing does not test terminal-stop policy
instead of work continuity; production policy is unchanged. All ten new tests
remain unexecuted because the required remote RCH launcher is absent.
