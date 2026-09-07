# Security and responsible deployment

## Supported status

**No production release exists.** The Rust reference model is not a sandbox, credential broker, durable ledger, live monitor, or containment system. It assumes trusted administrative inputs, runs in one process and thread, and deliberately performs no external effects. Deploying it around a powerful agent does not establish a security boundary.

A planned integration must identify exactly which effects are mediated, which credentials and operating-system boundaries enforce that mediation, and which bypasses remain. The comprehensive plan and [threat model](docs/THREAT_MODEL.md) define the intended obligations; they are not a claim that those obligations already hold.

## Reporting

For this design-only package, public issues are appropriate for synthetic counterexamples and documentation defects that reveal no live secrets. Do not put sensitive incident payloads or unpatched deployment credentials in an issue. If a published repository enables GitHub private vulnerability reporting, use that enabled channel for sensitive reports. This package does not claim that private reporting has been enabled or that a private intake endpoint currently exists.

Include the affected revision, threat assumptions, minimal redacted reproduction, actual versus expected outcome, and whether any real effect occurred. Preserve evidence under the relevant retention and privacy rules; do not repeatedly reproduce an irreversible effect.

## Supply chain and publication

The publishing script acts only through the user's locally authenticated GitHub CLI and configured Git identity. It checks for an absent target repository, refuses existing local Git metadata or ancestor worktrees, and never accepts or stores a token. It cannot undo a partially successful remote creation. Inspect the file set before invoking it: publication is a real public disclosure.

Required gates execute on operator machines through Cargo/DSR. No hosted Actions status is authoritative. Release requires a frozen source/toolchain/dependency/target closure and complete retained execution evidence; revision 0.3 is explicitly not release-enabled; the single local quality-gate execution of 2026-09-06 is quality evidence, not a release receipt.

## Privacy

Activation compression does not anonymize private data. Counterfactual research is isolated from production credentials and external effect channels. Redaction, retention, authorization, key management, independent audit anchoring and deletion are separate controls with separate evidence requirements. See [operations and privacy](docs/OPERATIONS_AND_PRIVACY.md).
