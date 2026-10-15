# Recover every retained learned message outcome

The learned publication operator's `--resume` now selects the original message
from the durable request book, rather than requiring a successfully admitted
`FrozenAction`. A policy-refused message is still a retained request: losing its
stdout receipt must not force a new request, renewed deadline or new inference.
This is scoped FA-014 recovery work, serving plan sections 8.4 and 16.4 and the
founding effect-boundary/evidence principles. Full packet acceptance stays open.

`FileOversight::learned_text_stream_message_request` is a historical data accessor.
It requires an existing key and uses the original source-only retry validator:
version, scope, complete stream profile, release kind, witnesses, full-frame
charge and retained deadline are checked. No output is rebuilt from today's
model state or audience prefix. A finish is not accepted as a message. Faulted
owners and absent keys refuse. The returned proposal is not an endpoint receipt,
permit, new request or proof that anything was published.

The existing guarded model/predictor replay, reserve, external floors and anchors
still precede live-owner recovery. The original source-only gateway recovers the
exact ticket; the existing receipt handler preserves refusal, cancellation,
confirmed execution and unknown liabilities. Terminal recovery reads no live
policy/helper assets and consults no logical clock. A denial remains a denial,
including the executable's nonzero exit status; it is not relabeled success.

Two regression functions cover legacy, required raw-residual and required owned-K/V
profiles. Paired scenarios recover a policy-refused request with no FrozenAction
and a real numerically generated, native-reviewed, independently human-approved
confirmed publication. Further cases reject absent/zero keys without writes and
retain the exact original deadline, numerical state, request disposition and
forecast sample count after live assets have been removed.

These Rust tests are authored but unexecuted: the required RCH launcher and Rust
toolchain are unavailable in this session. Source-preimage and whitespace checks
are not compilation or runtime validation. No production, calibration, performance
or full-workspace qualification is claimed. Moving receipt selection ahead of the
recovery fence is separate integration work; this first change does not yet make
invalid operator recovery attempts write-free.
