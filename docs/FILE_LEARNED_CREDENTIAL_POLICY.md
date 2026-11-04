# Credential policy binding in learned publication

The original learned configuration can require an exact nonsecret credential
policy through `FileLearnedConfig::with_required_credential_policy`. The policy
contains the original effect family, route, credential name and adapter-profile
generation. It contains no broker or provider secret, live credential permit,
credential-equality result, rotation generation or revocation state.

The [credentialed learned operator](LEARNED_CREDENTIAL_PUBLICATION.md) selects
this policy through an independent perimeter profile, preflights separately
provisioned live inputs and carries the original permit through publication.

## Independent registration before numerical startup

`FileCredentialPolicy::from_inventory` resolves the existing policy from an
independently supplied perimeter inventory, route binding, action scope and
target. It uses the original credential resolver and grants no authority.

Selecting that policy in a learned recipe does not install a guard. The existing
guarded learned constructors still require `FileCredentialRegistration` and
resolve its actual inventory before publishing the first canonical image.
Learned Enable requires the already installed policy to match the selected
requirement exactly, before it creates a numerical owner. An absent or different
guard therefore cannot be repaired by asserting a policy in the recipe.

The same check serves ordinary, raw-predictive, owned-predictive and mediated
learned startup. Existing sidecar, source, identity, joint-qualification,
forecast, publication-witness and recovery-reserve requirements remain in force.
The separate broker/provider pair must still produce an original owner-bound
`FileCredentialPermit`; neither the recipe nor the inventory supplies one.

## Exact historical identity

The bounded `FALCRD` configuration wrapper retains the complete original learned
recipe and exact credential-policy fields. Repeated selection refuses. Existing
unselected recipes retain their bytes and behavior, and wrapper order remains
part of the independently supplied recipe identity.

All learned historical readers already compare that complete configuration
before numerical replay. Omitted or changed credential policy therefore refuses
in recovery, receipt inspection and anchored review audit, including readers
that do not themselves accept a live credential. An audit cannot silently inspect
the selected history under a weaker numerical recipe. Wrong expectations do not
authorize staging cleanup, a recovery fence, a role or another publication.

## Rotation, revocation and publication

The original credential generation and revoked state remain dynamic lifecycle
state. Guarded recovery requires their independently supplied exact values through
`FileRecoveryRequirements`; rotation does not require rewriting the immutable
numerical recipe. Original generation checks and terminal revocation remain
authoritative, and reopening creates a new issuer that rejects an old permit.

Successful publication still requires the original automatic and human keys,
current source and publication evidence, valid target and deadline, and a freshly
bound credential capability when the native gate requires one. Historical
receipts, restrictive nonexecution and Stop/reconciliation do not obtain a new
effect right from this configuration requirement. Unknown effects retain their
original charge and cannot be resent through recovery.

## Scope

This connects existing L5 credential policy to the independent learned recipe
and its L6 historical readers, serving FA-011/014 and plan sections 8.3–8.8 and
16.4–16.9. The original durable publication journal remains the effect sink.
Perimeter declarations and both credentials remain trusted operator inputs.
This does not implement a remote provider protocol, encrypted secret storage,
cryptographic authentication or isolation from arbitrary code in the operator's
process. The distinct external file endpoint retains its separate recovery
contract. See [the durable credential guard](FILE_DURABLE_CREDENTIAL_GUARD.md)
and [driver credential mediation](FILE_DRIVER_CREDENTIAL_GUARD.md).
