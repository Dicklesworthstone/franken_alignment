# Credential-guarded learned publication

The learned operator can require the original credential guard at its actual
publication boundary. It uses the existing durable `FileOversight` owner, native
perimeter resolver, separate broker/provider capabilities and first-publication
driver. The effect remains the owner's canonical publication journal.

## Select the complete original recipe

The outer object has exactly three fields:

| Field | Value |
|---|---|
| `schema` | `"fa.learned-credential-publication/1"` |
| `credential` | Absolute normalized path to the independent credential profile |
| `recipe` | The complete original learned recipe object, with any selected checked-publication, identity and joint envelopes |

Canonical composition is `credential(checked(identity(joint(original))))`.
Each inner envelope is optional, and original learned recipe versions 1 through
6 remain supported. Duplicate or reordered wrappers, unknown fields, unsupported
schemas and invalid paths refuse. The original 64 KiB recipe bound remains; the
parser admits only one additional bounded nesting level. Unselected recipes
retain their original configuration bytes and live acquisition behavior.

Use this wrapped recipe with the existing commands and their original options:

```sh
supervise_publication create-learned-generated CONFIG CREDENTIAL_RECIPE REVIEWER_PROFILE
supervise_publication create-learned-generated CONFIG CREDENTIAL_RECIPE REVIEWER_PROFILE --actor-profile ACTOR_PROFILE
supervise_publication create-learned-generated CONFIG CREDENTIAL_RECIPE --resume
```

Continuation, separately reviewed Finish, resident sequences, historical receipts,
anchored review audits and Stop/Drain retain their existing command contracts.

## Independent credential profile

The closed profile declares the expected nonsecret policy, current native
credential epoch and separately provisioned live inputs. For example:

```json
{
  "schema": "fa.learned-file-credential/1",
  "policy": {
    "family": "publication",
    "route": "adapter:file-oversight",
    "credential": "broker-token",
    "profile_generation": 1
  },
  "epoch": {
    "generation": 1,
    "revoked": false
  },
  "inventory": "/operator/perimeter.json",
  "broker_credential": "/operator/broker.bin",
  "provider_credential": "/provider/expected.bin"
}
```

The policy fields must match the independently loaded original perimeter route,
including its declared credential name. The original resolver requires the
`fa.file-oversight-publication` profile, matching endpoint contract generation,
effect scope, `file_write`, broker-mediated credentials, blocked bypass and an
enforcement trust hop. An asserted policy does not install a guard.

For live admission, the profile and inventory consume the same aggregate learned
asset allowance as the original model, fit, tokenizer, monitor and other assets.
The profile is bounded to 64 KiB; inventory parsing keeps its original native
limits. Policy resolution precedes model loading. Native guarded construction
then independently resolves the registration again and installs the guard in
the first canonical image before learned Enable checks the exact policy.

New owners require active credential generation 1. Continuing owners require the
exact independently selected current generation and revoked state. A revoked
selection cannot start live execution; it remains available to historical and
terminal recovery operations under their original rules.

## Live inputs and capability custody

Before opening service sockets or creating/recovering an authority store, the
operator reads the broker and provider files independently. Each is an original
bounded regular-file input of at most 4,096 bytes. Bytes are used exactly as read:
there is no trimming, derivation of one input from the other, or encoded secret
inside the JSON profile. Missing, unequal, empty, oversized or aliased inputs
refuse; identical paths and hard-linked aliases cannot supply both sides.

Actual bytes consume the remaining original aggregate asset allowance. The
original deadline is checked around acquisition. A refused preflight leaves
creation or recovery retryable, with no new store, socket or recovery fence.

The prepared inputs are distinct native redacted capabilities. After the actual
owner exists, the original `bind_credential_pair` creates its non-cloneable,
owner-bound permit. A single live session owns that permit. Secrets and permits
do not enter the numerical recipe, journal, helper packet, human review or actor
response. Error messages do not include credential bytes.

The original native/process helper review, automatic permit, human approval,
source freshness, target, deadline and any configured publication witnesses
remain mandatory.
Only the first-publication driver receives the borrowed credential capability.
A missing or foreign permit cannot reopen a failed send phase; subsequent driver
steps reconcile the original obligation and preserve any unknown charge.

## Waiting and resident requests

The credential selection composes with `fa.supervised-witnesses/4`. Healthy
producer lag retains the same prepared action, review, human approval, finite
retry allowance and credential permit. Each retry reacquires policy, feed and
capture evidence under the original freshness rules. No retry reopens credential
files or renews the human approval, action deadline or credential epoch. Stop and
actor cancellation keep their original checkpoints.

A resident sequence declares its complete entries before startup. Every entry
must retain the same policy, epoch, inventory selection and credential input
paths. The same session retains one permit across Message, Message and separately
reviewed Finish; each request still needs its own original review and human key.
Removing provisioning files after binding does not revoke a capability. Rotation
and revocation remain the original native lifecycle transitions, not file edits
or changes to a later sequence entry.

## Recovery, receipts and anchored audit

The original learned recipe now includes the immutable credential policy through
[its native policy binding](FILE_LEARNED_CREDENTIAL_POLICY.md). All construction
and recovery modes retain that exact policy, including raw/owned forecasts and
topology. Omission or any policy-field change cannot reopen the same history.

Owner recovery, read-only receipt recovery and Stop/Drain also require the exact
independent current credential generation and revoked state. These checks precede
staging cleanup, a recovery fence and role release. A valid reopen fences old
keys and rejects an old owner-bound credential permit. Outstanding ambiguous
effects retain their charge; recovery does not authorize resending.

Historical commands read the selected profile and original historical model
assets, but do not open the live perimeter inventory, broker/provider files,
current evidence or helper inputs. A historical receipt supplies no new effect
capability, even when all those live inputs have disappeared.

Anchored review audit has its original historical meaning: it binds the complete
immutable numerical recipe, including credential policy, and the independently
retained history anchor. Current generation/revoked expectations do not change
that past review. An epoch-only profile change therefore leaves the same anchored
audit unchanged; a changed or omitted immutable policy refuses. The audit does
not attest current credential possession or current permission to publish.

## Scope

Guarded startup selects the original plain, raw-predictive or owned-predictive
native constructor in a separate private call frame. Each path retains the same
independent guard registration, terminal reserve, generation and separate role
custody. This avoids aggregating all alternative bootstrap temporaries in an
already-deep unoptimized startup caller; it grants no weaker credential route.

On 2026-10-24, source `83cd4443` passed all 42 original checked-publication and
computed-identity functions, including the nine credential functions, on the
worker where the unchanged credential-history function had independently aborted
with a stack overflow at `a985f9cd`. The original test profile and stack settings
were retained. The complete 744-function operator run also passed at this exact
source, zero failed/ignored/filtered. This is scoped execution, not a static
stack/RSS bound or full project qualification: lint and dependency admission
still fail. Global gates are recorded in implementation status below.

This is a bounded integration of the original journal publication sink under
FA-011/014 and plan sections 8.3–8.8 and 16.4–16.9. The inventory and both inputs
are trusted operator provisioning. Secret material occupies ordinary process
memory, and equality is not a cryptographic authentication protocol. The feature
does not claim an encrypted vault, hostile-process isolation or a remote provider.
The distinct external file endpoint still needs its own durable send/receipt and
whole-process recovery integration. Current execution evidence is recorded in
[implementation status](../IMPLEMENTATION_STATUS.md).
