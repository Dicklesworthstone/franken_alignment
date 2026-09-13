# Automatic containment of a nonquiet owned decoder

Consumer: the trusted host using OversightBroker's owned monitored decoder.
This connects the plan's configured cheap-tripwire consequence (§9.7–9.8) to the
existing stop-before-effect and endpoint-outcome protocol (§8). It adds no
congress, replacement ledger, actor command, executor, dependency or background
watcher. Source and regression tests are uncompiled/unexecuted; no production
qualification or task closure is implied.

## Explicit policy, original transition

After owning a currently quiet MonitoredSampledDecoder and before any proposal or
review, install HostedStopPolicy with enable_hosted_stop. The policy identifier,
generation and stop-operation key are explicit trusted inputs. There is no edit,
disable, source-swap or actor-selected exception. Existing manual containment and
paired-reset behavior remains the default when this policy is not installed.
A reviewed warm prefix can be attached; this is not a claim that the stop policy
was active before bootstrap.

Both original hosted inference methods enforce the policy before and after the
numerical operation. A nonquiet review or admitted numerical failure invokes the
ORIGINAL request_stop, closing admission, advancing the revocation floor and
cancelling undispatched attempts. Only their reservations are refunded. A
computed-but-held token retains its actual KV state, draw and monitoring work.
An admitted arithmetic failure can trigger containment even if the original
numerical transaction did not advance its token position.

The first HostedStopIncident retains the real cause, policy, actor revision,
source stream, position, draw count, monitoring work and original stop receipt.
Alarm, exact threshold equality, budget exhaustion and numerical failure stay
distinct. Operational uncertainty is not credited as a detected policy violation.
No token IDs, logits, raw cache or random words enter that observation. An
incident cannot be supplied to a receipt or permit interface.

Stale revisions, invalid tokens, inadequate numerical/sampling preflight and
other rejected calls leave a still-quiet owner unstopped. Repeated enforcement
returns the original stop receipt, not another epoch transition. An earlier
manual stop is respected rather than renamed. Failure of the original stop is
retained and returned; a later explicit enforcement can retry without replaying
inference. Historical trigger fields never refresh to a more convenient cause.

## Reset and failure boundaries

An installed policy is also checked before a paired hosted reset. A trip cannot
be reset away. A failed admitted replay which poisons the numerical owner applies
the policy before returning its error; failed preflight does not. A successful
healthy reset retains the same policy, and original incident-driven suspension
is not relabelled as a new numerical fault. The original reset implementation,
replay budgets and state-comparison checks are otherwise unchanged.

No unwind catcher or asynchronous worker is added. The monitored owner already
withdraws eligibility before admitted computation and poisons itself on unwind.
A caller that catches an unwind must resume a supervising boundary, such as
enforce_hosted_stop, to perform the control transition. A stopped process cannot
run this method. This is not hostile-process containment or whole-process
recovery. Probe and parameter authentication and detector quality remain
independent requirements.

## Local stopping is not endpoint stopping

The broker does not own its publication endpoint. This automatic transition
performs no endpoint I/O and cannot claim that already-issued envelopes stopped.
Use the original progress_stop to acknowledge a fresh endpoint fence and settle
original obligations. An execution before that barrier remains executed and
charged. A missing request is refunded only after endpoint sealing. Retention
loss and irrecoverable outcomes prevent a false drained result. The supervising
endpoint owner supplies that operation without requiring another helper or human
decision; this broker API never fabricates endpoint evidence.

## Regression source

Nine public regression functions exercise actual numerical inference with both
publication key profiles; ordinary quiet publication; reserved-action cancellation;
unknown and already-executed outcomes; unchanged healthy preflight; threshold and
budget distinctions; arithmetic overflow; frozen configuration and the unchanged
manual-reset control; healthy and failed replays; and intentional suspension.
One compile-fail example distinguishes an incident from an endpoint receipt.
Synthetic weights and helper verdicts are not trained-detector qualification.
Rust compilation, rustfmt, tests and the required RCH gate have not run here.
Existing tests, registry gates, manifests, licenses and beads are not altered.
