# Explicit credential selection for brokered routes

Perimeter inventory version 2 can assign a different broker credential to each
route in the same effect family. Both existing concrete consumers use the same
checked resolver: the credential broker for real disposable file endpoints and
the native credential guard used by learned journal publication.

## Closed version 2 schema

The root selects `"version": 2`. Each route with a broker-mediated credential
must use exactly this shape:

```json
"actor_credential": {
  "kind": "broker_mediated",
  "credential": "provider-a-token"
}
```

The name must identify a credential declared with `"holder": "broker"` in that
route's own family. The route must also declare `"mediation": "brokered_effects"`.
A missing name, unknown name, actor-held name, cross-family reference or mismatched
mediation refuses during inventory admission. Other credential dispositions keep
their original closed shapes; they cannot carry a broker selection as an extra
field.

For example, one family's credentials can be:

```json
"credentials": [
  { "credential": "provider-a-token", "holder": "broker" },
  { "credential": "provider-b-token", "holder": "broker" }
]
```

Its first brokered route names `provider-a-token`, and its second names
`provider-b-token`. Every other required inventory field remains mandatory:
scope, effect family, declared trust domains, route identity, effect kind,
adapter profile and generation, trust path, threat, mediation, bypass disposition
and residual nonclaims. The name is nonsecret configuration, not the credential
bytes or proof that an operator possesses them.

All original limits remain in force. Each explicit route credential name counts
again toward the family's aggregate text allowance, in addition to its declaration
in the family credential list. Version 2 does not increase the inventory byte,
item, string, family, route or trust-path ceilings.

## Version 1 keeps its original meaning

Version 1 still accepts only `{"kind":"broker_mediated"}` for this disposition.
Adding the version 2 field to a version 1 document refuses. Its effect resolver
still requires exactly one broker-held credential in the selected family.
A version 1 family with multiple broker credentials remains valid classification
data but cannot drive a concrete effect through ambiguous credential selection.

There is no fallback from an invalid version 2 name to the version 1 rule. Neither
route order nor credential-list order chooses a credential. The public
`ActorCredentialDisposition::BrokerMediated` variant remains unchanged;
`RouteMetadata::broker_credential_name()` exposes only the explicitly selected
version 2 name. Effect consumers continue to use
`LoadedPerimeterInventory::broker_credential_for_route` for checked resolution.

## Existing effect consumers

`CredentialBroker` validates the selected name at attachment, delivery and
endpoint-worker reopen. Its `fa.disposable-file-publication` profile, original
endpoint identity, dispatch envelope, reviews, any configured second key, fences, deadlines,
retention and receipt checks remain in force. Two routes can use independently
provisioned broker/provider pairs and distinct file endpoints within one family;
an envelope for one endpoint cannot be exercised against the other.

The learned operator continues to use `fa.file-oversight-publication`. Its
independent credential profile must name the credential selected by the route.
The original native resolver checks that selection before model loading, guarded
bootstrap independently installs it, and the exact policy enters the immutable
learned recipe. A changed route or credential name cannot reopen that history
under a different policy. Historical loading still requires no live inventory
or credential file.

The inventory version is not a new effect credential, publication permission or
numerical model identity. The same resolved policy has the same native policy
fields. Actual secret agreement still uses the original independently supplied
`BrokerCredential` and `ProviderCredential` capabilities.

## Scope

This is explicit routing within the existing trusted-operator perimeter contract,
serving FA-011/014 and plan sections 4.4, 8.1–8.8 and 16.9. It does not authenticate
credential names against a remote provider or create an encrypted secret vault.
The real file endpoint retains its surviving-controller recovery contract; the
learned operator retains its canonical-journal effect sink. Whole-process external
send/receipt recovery and hostile-process containment remain separate work.

See [broker/provider separation](CREDENTIAL_BROKER_PROVIDER_SPLIT.md),
[file-endpoint recovery](CREDENTIAL_BROKER_RECOVERY.md) and
[learned operator selection](LEARNED_CREDENTIAL_PUBLICATION.md).
