# Two-key execution and delayed delivery

Implementation scope: the in-memory `fa-reference` publication protocol. This is the execution-boundary part of FA-121 / plan section 9.11, serving FI-A06, FI-A13 and FI-A16. It does not complete the human-member adapter, human authentication, durable co-signing or production effect mediation. The FA-121 beads remain open.

## Contract

A successful congress decision and a separately provisioned human approval are both required by the protected oversight broker. Consuming these keys at broker dispatch does not extend either validity window. First publication at the registered endpoint must occur before **both** the original action deadline and the human approval expiry. The interval is half-open: execution at the expiry tick is refused. An endpoint clock preceding the recorded approval time also refuses without claiming terminal nonexecution.

`PublicationRequest::deadline()` remains exactly the reviewed action's deadline. `execution_deadline()` returns the narrower bound. The request carries only the second key's request ID, reviewer ID, issue tick and expiry tick. No review input, witness, helper vote, actor checkpoint or argument to the reviewer is added to the endpoint projection. Copying this metadata does not mint a key or a dispatch envelope.

The broker retains the approval alongside its original action record. Recovery queries reproduce it exactly; receipts bind it through complete request equality; receipt and query admission compare it against the retained record. A restart cannot silently drop the shorter deadline or replace the original key identity.

An endpoint that has already published returns its retained executed receipt even after the keys expire. It never turns a real execution into a refundable failure. A first delivery after either deadline instead records `NotExecuted { DeadlineElapsed }` without changing payload, resource version or stream prefix. Only that terminal endpoint evidence permits the original resource charge to be returned, once. The human key stays consumed in either case. A missing status, elapsed clock or local cancellation alone never refunds an unknown effect.

## Verification status

Added public controls cover first delivery immediately before, at and after human expiry; unchanged frozen-action bytes; exact receipt metadata; delayed acknowledgment after real execution; missing-status accounting; dispatcher restart and sealing; and the unchanged single-key profile. Constructor controls include zero IDs, empty/reversed windows and `u64::MAX` tick boundaries. A compile-fail example checks private metadata construction.

These additions have **not been compiled, executed, formatted by rustfmt or qualified through RCH in this editing environment**. It has no Rust toolchain or RCH access. Source review is not execution evidence. Existing dated execution receipts and test counts are unchanged and do not qualify this increment. No dependency, runtime or release admission is changed.
