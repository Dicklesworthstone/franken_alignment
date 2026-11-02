# Unambiguous v1 broker credential resolution

The v1 perimeter inventory declares credentials at family scope rather than
binding each brokered route to a named credential. That is sufficient for
classification, but not for effect execution when a family declares multiple
broker credentials: choosing one would add authority not present in the route.

`LoadedPerimeterInventory::broker_credential_for_route` is the execution-only
resolution rule for this schema. It first requires the exact route to be
`brokered_effects` with `broker_mediated` actor credential disposition, then
requires exactly one family credential whose holder is `broker`. Zero broker
credentials is incomplete; two or more is ambiguous and returns `Binding`.

Generic inventory loading and `route_for` remain backward compatible. A
multi-broker family can still be inspected, audited and classified; it simply
cannot drive the concrete credential broker under v1. A future schema can make
route-to-credential identity explicit without reinterpreting old documents.

`CredentialBroker` consumes this execution-only lookup both at initial attachment
and after endpoint reopen. Its immutable inventory therefore cannot switch the
effect path to another family credential. Delivery also rechecks the lookup along
with mediation and bypass status before presenting the independently supplied
broker credential to the provider-side expectation.

This change does not authenticate credential names or secret bytes. The existing
broker/provider capability split still supplies the actual secret agreement, and
the inventory remains trusted operator configuration. It also does not upgrade a
cooperative/observe-only route, create a permit, or infer that the inventory is a
complete deployment perimeter.

Source coverage pairs a valid single-broker route with an otherwise identical
multi-broker family that remains parseable but cannot construct a concrete effect
broker. The required RCH runner is unavailable in this environment, so Rust
compilation, formatting, Clippy, tests and doctests remain unexecuted. No Beads or
production-gate status is changed.
