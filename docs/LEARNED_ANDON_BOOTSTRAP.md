# Independent accounting at learned startup

The original learned owner can install its evaluator, Andon accounting and
terminal recovery reserve in one first canonical image:

```rust,ignore
let (owner, roles) =
    FileOversight::create_andon_guarded_with_learned_generation_reserved(
        directory,
        profile,
        &guards,
        credential_registration,
        evaluation_protocol,
        &andon_config,
        learned_config,
        recovery_reserve,
    )?;
```

This is the FA-105 composition serving FI-A08, FI-A13 and FI-A18 under plan
§§9.9, 13.4 and 16.4. It uses the existing numerical recipe, native evaluation
and Andon reducers, guarded bootstrap, reserve admission and canonical writer.
It introduces no journal format or dependency.

Every declared guard and the complete learned recipe are checked before storage
creation. A recipe-owned policy source must equal the independent declaration;
the original learned Enable installs that source exactly once. Required raw or
owned pre-output predictors need their distinct role-preserving constructor and
refuse this evaluator-only lane. The complete first-image event inventory must
leave the selected terminal recovery tail. The final canonical encoder checks
actual bytes with the store identity.

The one canonical replacement must acknowledge before returning the owner and
separate human, evaluator and selected oversight roles. The constructor does
not compute a token, observe time, acquire fresh source evidence, assert an
identity match, label a case or approve publication. Existing directories cannot
be retrofitted or reset through this creation API.

After startup, an independently labeled benign Hold is charged by the original
Andon ledger. Pending, censored and violation outcomes remain distinct. An exact
assessment retry cannot duplicate the charge. Labels do not provide the separate
human key or restart a held effect. Accounting windows and costs remain native
history; this constructor adds no recovery protocol or live endpoint by itself.

Six public regression functions in
`tests/learned_guarded_recovery/evaluated_bootstrap/andon.rs` cover complete
role retention, exact reserve capacity, malformed input refusal before storage,
recipe-owned source identity, foreign/censored/benign labels and idempotent
costs, and actual publication requiring generation plus both review keys.
They reuse original numerical and durable APIs; fixture judgments are not
empirical detector evidence.

The required RCH command could not start because the runner was unavailable
(exit 127). The workspace subsequently disconnected. These sources received
manual review; the new functions have not been parsed, compiled, formatted,
run through Clippy or executed in this session. FA-105 remains in progress.
