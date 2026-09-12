# Operator-file evidence and current-input capture

## Concrete source

`oversight::evidence_source` reads one explicitly provisioned operator file into the existing Snapshot and CommitteeInput contracts. It does not accept an actor-supplied path, fabricate helper judgments, select an automatic permit, or replace a failed read with cached evidence. This is scoped reference progress for FA-017/FA-057/FA-081 and plan 7.6, 9.3, 17.1 and 17.7. The source consumer is the existing supervised actor-to-effect driver.

The version-one JSON document has exactly `version`, `source`, `generation`, `scope`, `semantic_epoch`, `complete`, `values` and `contexts`. Source, generation, semantic epoch, scope IDs and value-map keys are canonical unsigned decimal strings. `version` is the JSON integer 1. Scope contains tenant, principal, run, branch, authority and the literal purpose `effect`. Values and per-member context payloads use lowercase hexadecimal; context-map names are exact committee identities. The existing strict parser rejects duplicate keys, excessive nesting and malformed input. Unknown fields, wrong types, noncanonical IDs, overflow and malformed hex refuse. `EvidenceSnapshot::encode` is the producer-side encoder.

A file is a complete snapshot only relative to the producer's declared domain. The `complete` flag is retained exactly, including false; successfully reading a regular JSON file does not establish that an external database, filesystem or environment was completely observed. Snapshot authenticity, producer correctness and temporal coherence remain host obligations.

## Binding and privacy

FileEvidenceSource freezes an absolute operator path, source ID, full effect Scope and byte ceiling. Each read opens the current file anew. It rejects wrong namespaces, descending source generations, descending semantic epochs, and changes to any parsed content under the same generation. Equal content with different JSON whitespace remains equal. A new generation is retained even when its complete flag is false. Old returned snapshots remain immutable historical data, not automatically current observations.

A failed read sets current eligibility to unavailable while retaining the surviving generation floor. It never returns the old successful snapshot as the result. Restoring the exact prior file may make that source readable again; consumers must separately invalidate live approvals on the intervening outage. A same-generation substitution cannot acquire a new meaning by being retried. The source-version floor survives only with this source object, not loss of the whole process.

inputs_for builds each actual helper input from the original exact action frame, its registered question, and a framed context containing source/version/scope/member identity followed by that member's supplied context bytes. It uses the original ActualHelperInput and EvidenceViewManifest validators. The private Snapshot value map and other helpers' contexts are never automatically copied. Context bytes are explicitly host observations, not fabricated source-span claims about an unseen document. All roster members must have a context, even when its payload is intentionally empty; an absent member is not silently dropped. A generation change changes the submitted context frame even when its payload bytes are identical.

reference_root is an explicitly non-cryptographic 32-byte reference label containing source, generation, tenant and authority IDs. It is not SHA-256, a signature or a content commitment. Full action scope and actual inputs remain separately bound by the existing reference sessions. Source IDs must be unique within the operator's registered domain.

## File and resource assumptions

The producer must publish immutable files with atomic replacement, not edit the open file in place. The selected file and every ancestor must be controlled by the operator and outside actor write authority. Obvious symlinks and nonregular files refuse, but path checks do not establish security against a malicious parent or mount race. This module implements no filesystem watcher, authentication, encryption, durable anti-rollback storage, operating-system sandbox or native asynchronous runtime.

The file ceiling is 4 MiB, with smaller per-source limits. There are at most 256 private keys, 64 KiB per value and 512 KiB total private values, plus 64 KiB per helper context and 512 KiB total context bytes. The existing roster, identifier and 1 MiB complete CommitteeInput limits still apply. Aggregate input size is checked before constructing all helper submissions. The parser temporarily owns hexadecimal text in addition to decoded bytes, and helper inputs copy their selected context. These are logical input bounds, not measured peak memory, latency or syscall counts. Each call rereads and parses the file; it is not constant-time change detection.

## Verification status

The first increment includes ten Rust test functions, including a Unix symlink test. Positive controls cover exact binary/Unicode interchange, private-policy and peer-context separation, identical rereads, version advancement, exact size boundaries and complete=false. Negative twins cover scope/roster substitution, same-version equivocation, semantic rollback, malformed and every truncated document, missing files, unknown fields, oversized values and unchanged retained floors after failure.

These tests are unexecuted source. Cargo, rustc and a configured RCH runner are absent in this editing environment. Compilation, rustfmt, Clippy and the repository gate remain pending. No bead is closed and no historical execution receipt qualifies this change. No production dependency or alternative executor was added.
