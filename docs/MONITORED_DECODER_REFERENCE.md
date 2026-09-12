# Original decoder execution with compulsory residual review

Source status: implemented Rust source, not compiled or executed in this editing
session. No execution gate, detector qualification, or bead closure is claimed.
The existing RCH-only verification requirement remains in force.

## Consumer and actual behavior

The consumer is a trusted host running the original CPU reference decoder from
explicit parameters or compatible imported SafeTensors. Previously it could run
that decoder and separately invoke a refinement monitor, but the two operations
were not coupled. `activation::monitor::decoder::MonitoredDecoder` now owns the
numerical session and runs the existing source-checked codec/probe monitor on
every layer's computed residual before returning that step's logits or permitting
another token through this API. This is a narrow composition of the existing
capture, inference and probe implementations, not a second numerical backend.

The contract is explicitly **residual-only, every layer, every consumed token**.
It does not claim to review the model's entire activation state, query tensors,
KV values, token semantics, external tools, or all possible dangerous behavior.
The reference composition serves the founding activation-observation and economical
sidecar integration requirements; it does not satisfy their research or production
gates. `NoAlarm` remains an observation, never a permission or a safety certificate.

`DecoderModel::residual_contract(layer)` exposes the actual immutable contract
installed by the original model constructor. The monitored constructor requires
exactly one monitor per layer, with matching profile and dimensions. There are no
implicit probes, skipped layers, shape guesses, or same-size tap substitutions.
Actual step frames are checked for the complete layer roster, profile, dimension,
stream, sequence and position before any monitor runs. Callers cannot submit an
arbitrary public `DecoderStep` to satisfy a review or attach an already advanced
session and skip the prompt history.

## States, outputs and work

Only an all-layer `NoAlarm` report returns `MonitoredStep::Released(ReviewedStep)`.
The first alarm, threshold equality, budget exhaustion or unresolved result
returns `Held` with its report and an explicit unreviewed-layer count. Later
layers need not be evaluated after a hold. There is no raw step/logit accessor on
the held result, and no mutable inner session, checkpoint, reset or resume escape
hatch. A held session cannot advance another input or greedy token. The decoder
model is not returned by the wrapper.

The token whose review holds has already been numerically computed. Its private
KV cache is not rolled back. Its work remains included in `decoder_work()`; the
interface does not refund that computation or imply that the token never ran.
Preflight failures for stale position, invalid token or inadequate numerical
budget leave the session unchanged. After computation begins, an error latches
`Failed`, and the object is poisoned before execution so a caught unwind cannot
resume unreviewed state. Out-of-memory process aborts are not recoverable errors.

`RefinementMonitor::analyze_with_budget` intersects an additional allowance with
the monitor's existing fixed limits. The original `analyze` keeps its original
behavior. `MonitoredDecoder` supplies a shrinking session allowance across layers
and tokens. Neither per-frame calls nor subsequent token calls reset it. Encoded
bytes include codec headers and padding; probe/codec coordinates retain the
existing definitions. Reported monitoring work covers completed reports, including
zero-work BudgetExhausted reports; it does not invent costs for an errored monitor
call. Such a call cannot be retried on this session. Decoder and monitoring work
are separate. These limits do not bound wall-clock latency, allocator usage, RSS,
blocking I/O, or external effect rights.

## Strict configuration

`MonitoredDecoder::from_json(model, stream, bytes)` consumes the actual immutable
model and binds schema `fa.decoder-monitor/1`. The stream is supplied out of band.
Configuration is capped at 1 MiB, depth eight, 262,144 syntax items and 256-byte
strings. It requires explicit model/tokenizer/profile identities, monitor
generation, aggregate allowance, complete layer inventory, local allowances,
progressive ladders and linear probes. Probe coefficients, bias and threshold
are finite binary32 values obtained by parsing JSON decimal numbers; rounding,
including underflow to zero, is part of this import profile. IDs are positive
integers, not coerced strings or floating-point numbers.

Unknown fields, duplicate keys, mismatched epochs, missing/duplicate layers,
incorrect probe dimensions and invalid ladders refuse. The same original
LinearProbe and RefinementMonitor constructors perform final admission. No JSON
field opens a path, downloads data, executes code, authenticates a model, or grants
a capability. The retained `decoder_monitor_quiet.json` is a deliberately synthetic
nonzero-coordinate control, not a trained detector or a meaningful safety policy.

## Verification source and nonclaims

Nine integration tests cover uninterrupted versus monitored prefix/32-token
continuation, source-derived alarm/equality controls, late and early holds,
complete monitor binding, shared allowance across layers and tokens, local-limit
intersection, unchanged preflight refusals, sticky arithmetic failure and imported
SafeTensors. Six configuration tests cover exact identity epochs, strict schema,
finite coefficients, roster/ladder admission, every truncated JSON prefix, size
limits, persistent aggregate allowance and original imported-model numerics.
Two compile-fail examples describe the no-Permit and no-mutable-session boundaries.
None of these Rust tests or doctests has executed in this session.

The wrapper is a trusted-process numerical API. A caller that independently
retains the raw DecoderModel can run it elsewhere. This is not hostile-process
containment, external effect mediation, authenticated transport, durable recovery,
a production release, calibrated detection, or evidence that synthetic probes
find harmful behavior. Existing actor/effect authority remains separate and is
not weakened or bypassed. No dependency, runtime or manifest edge is added.
