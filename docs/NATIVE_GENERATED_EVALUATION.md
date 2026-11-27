# Independent evaluation for native generated publication

Native generated text can now share one original owner with independent helper
evaluation and Andon accounting. This composes the actual source, decoder,
tokenizer, publication and evaluation reducers. Configuration does not supply a
case, a label, a source observation or permission to publish.

This implements part of FA-105/049 and the live-evaluation composition, following
plan §§9.9, 13.4, 17.1 and 22.3–22.4 and FI-A08/A13/A18. The work remains in
progress pending execution qualification.

## One atomic initial image

Select `FileGeneratedEvaluationConfig` from
`observed::stream::generated::checked::source::evaluation`. Its mandatory input
is the independent `EvaluationProtocol`; `with_joint` selects the existing joint
promotion policy, `with_andon` selects the original accounting configuration,
and `with_campaigns` selects the original independent policy-campaign guard.
The builder accepts each optional selection once. The original native reducers
validate the selected configuration before storage is created. The original
journal admission law also checks reserve dimensions and ensures the complete
bootstrap event count leaves its terminal reserve intact. Canonical encoding
still checks actual byte usage against the final store identity; byte encoding
or storage can fail without returning any live role.

`FileOversight::create_generated_text_stream_from_source_evaluated` takes the
same source, decoder, tokenizer and `GeneratedSourceProfile` as the original
source-bound constructor, plus this evaluation configuration. It preserves the
complete compatible plain or checked publication selection and its explicit
recovery reserve. Owned-round `JointPromotionPolicy` is an optional additional
evaluation guard. Held-out joint qualification is a separate, exclusive native
lane: `GeneratedSourcePublication::Joint` is refused before storage creation.
No stronger selected guard is replaced with an ordinary mode.

The sole first canonical replacement contains all bootstrap inputs. Only its
acknowledgment releases the owner and `FileEvaluatedOversightRoles`; an uncertain
write returns no role, even if the complete image reached disk. The human
reviewer, evaluator and explicitly selected policy governor remain separately owned. There is no public live-owner
role getter, imported score, second journal or new event format.

The positive path still performs actual native inference, takes a fresh source
capture, applies the original full-input congress and requires the independent
human key before publication. The evaluator can discover an applied case and
provide an independently owned assessment. It cannot approve publication or
change control/authority. A confirmed benign Hold charges the original Andon
window and may open its original calibration incident; it does not loosen a
policy or retire a helper.

## Recovery with independent contracts

`begin_open_andon_guarded_generated_text_stream_from_source_anchored` returns
`FileAndonGeneratedSourceRecovery`, a sealed wrapper over the existing bounded
generated-source replay cursor. Its synchronous `open_...` counterpart uses the
same cursor and sole finalizer.

The caller must retain and select the exact source/decoder/tokenizer/stream,
publication mode and reserve, complete guard inventory, effective policy,
credential epoch, evaluation protocol and optional joint policy, original Andon
configuration and complete current window inventory. Independent journal,
control, authority and evaluation floors remain mandatory inputs. A separately
retained exact `FileHistoryAnchor` is mandatory; matching a prefix does not skip
the original suffix replay or authenticate the latest head.

The wrapper holds the same locked canonical image throughout. It checks the
anchor and static contracts before numerical replay, advances a bounded number
of original events, and reports Ready only after the final accounting and floor
checks pass. Contract failures latch and cannot escape through `finish` or Stop.
It never exposes its inner cursor or an early evaluator role.

Successful `finish` acknowledges the original single recovery Fence before
issuing fresh human, evaluator and explicitly selected governor custody. Original numerical/RNG spending,
pending or censored cases, labels, Andon receipts/incidents and unresolved effect
liabilities survive. Old evaluator/ticket brands and human approvals do not.
New generation still needs fresh source/time and explicit native resume. A late
benign assessment needs fresh native time and remains charged to the original
case occurrence window.

Cancellation leaves canonical and staged bytes unchanged. The independent
terminal admission Stop and authenticated recovery Stop paths use the original
verified reconstruction and reserve; they do not first issue a live owner or
ordinary recovery Fence. A finalization/storage failure returns no live roles.

## Retain independent policy governance in the same owner

`FileGeneratedEvaluationConfig::with_campaigns(FileCampaignRequirement)`
selects the original policy-campaign guard alongside evaluation and optional
joint/Andon accounting. Duplicate selection refuses. The same original campaign
reducer validates the exact bounds before storage exists; the complete bootstrap
then passes the original event, terminal-reserve and path-bound byte admission.

The acknowledged `FileEvaluatedOversightRoles` contains the independent evaluator
and, when selected, a separately held `oversight.policy_governor`. The governor
can approve an original replay campaign; it cannot supply an evaluation label or
a publication key. The evaluator can label original cases across policy changes;
it cannot approve a campaign or publish. The human key remains separately held.

Guarded generated-source recovery requires the independently selected campaign
inventory and effective policy together with the original evaluation/Andon
contract and history floors. Neither subsystem can be omitted on recovery.
One acknowledged Fence issues fresh roles while preserving original cases,
censored assessments, numerical spending, campaign use and effect charges.
Old roles and tickets do not become valid on the recovered owner.

Authored regressions cover compatible plain/checked and marginal/joint modes,
exact complete-image event/byte limits, all five original storage barriers,
invalid/duplicate selections, and actual native publication followed by policy
promotion, delayed independent labels, anchored recovery and fresh publication.
They remain uncompiled and unexecuted because the required RCH launcher is
unavailable. This API composition does not qualify detector accuracy or install
the six full native risk-theater feeds. Live-service support has its own explicit selection in the workflow contracts
linked below.

## Verification and scope

The authored bootstrap matrix covers 20 compatible mode/configuration
combinations and separately rejects all four evaluation configurations with a
held-out joint publication guard. Causal regressions cover malformed reserve
dimensions, insufficient ordinary event capacity and absent helper model
generations before storage creation, each with a valid nearby control. Other
regressions cover real generation and two-key Message/Message/Finish publication,
applied Hold accounting without resampling or evaluator-issued effect rights,
equal-case foreign custody, censored-label recovery, pending numerical resume,
independent floors and inventory mismatches, cancellation at every boundary,
changed canonical state, all five original Store failure barriers and exact
terminal Stop retries. Synthetic model/helper choices test native plumbing and
contracts; they are not evidence of learned detector accuracy.

Independent source review completed. The earlier constructor increment passed
its scoped rustfmt check; that evidence does not qualify the new campaign
composition. The required RCH launcher is unavailable in this development
environment, so formatting, compilation, Clippy and runtime execution of the
new changes remain unverified. The Bead remains in progress. Neither this composition nor the
existing score kernel completes FA-125's six native risk-theater predicates.

The native API supports separately guarded recovery. Resident generation can
select both original live roles through the
[governed evaluation workflow](RESIDENT_NATIVE_PUBLICATION.md#keep-evaluation-active-through-native-policy-promotion).
Other live executable support is documented in [the evaluator workflow](LIVE_ANDON_EVALUATION.md) and
[native text publication](NATIVE_TEXT_PUBLICATION_SERVICE.md); unsupported live
mode combinations continue to refuse explicitly.
