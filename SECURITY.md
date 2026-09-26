# Security Policy

## Scope

OurDAO is **testnet-stage only**. There is no mainnet deployment, and the contract has **not been externally audited** (see the [Security notes](./README.md#security-notes) in the README). Keep that in mind when assessing severity — a finding here is real and worth reporting, but there is no live mainnet value at risk today.

This policy covers `ourdao-contracts`, the Soroban smart contract that is the single source of truth for all OurDAO state.

### In scope, and most valuable

- **Fund-loss paths** — anything that lets treasury or member funds be moved, locked, or miscounted outside the intended rules (loan issuance/repayment, treasury withdrawal, membership exit, yield distribution, default handling).
- **Authorization bypasses** — anything that lets one member act on another's behalf, or lets a non-admin exercise an admin-only entrypoint.
- **Governance/vote integrity** — anything that lets a vote be miscounted, double-counted, or a proposal be approved/executed outside its stated rules.

### Out of scope (known, already tracked)

These are known, accepted limitations, not novel findings — please don't spend time writing them up:

- **No upgrade path.** The contract is immutable by design; see [`docs/MIGRATION.md`](./docs/MIGRATION.md) and the README's [Known limitations](./README.md#known-limitations).
- **Testnet-only deploy tooling.** Tracked in [#30](https://github.com/ourdao/ourdao-contracts/issues/30).
- **No external audit yet.** Preparation is tracked in [`docs/AUDIT_PREPARATION.md`](./docs/AUDIT_PREPARATION.md) and on the [Roadmap](./README.md#roadmap) — an audit is planned before any mainnet consideration.

If you're unsure whether something is a known limitation or a genuine finding, report it privately anyway (see below) — that's a cheaper way to resolve the ambiguity than either of us guessing in a public issue.

## Reporting a vulnerability

**Do not open a public issue.** Use GitHub's [private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability) on this repository (Security tab → "Report a vulnerability").

A good report includes:

- **Reproduction** — the exact call sequence (or a test) that triggers the issue.
- **Impact** — what an attacker gains: funds moved, votes miscounted, an unauthorized action executed, etc.
- **Affected entrypoint(s)** — the specific contract function(s) involved.

### Response commitment

We aim to acknowledge a report within **5 business days**. This is testnet-stage, volunteer-maintained software — treat this as a realistic commitment, not a guaranteed SLA.

### Disclosure

We follow coordinated disclosure: once a report is triaged and (if applicable) a fix has landed, we'll work with the reporter on when and how to disclose publicly. Reporters are credited by default (in the fix's commit/PR and any advisory) unless they ask to remain anonymous.

## Other repositories

This policy currently covers `ourdao-contracts` only. `ourdao-backend` and `ourdao-frontend` carry the same inline reporting instructions today and should get their own `SECURITY.md` — tracked as follow-up work outside this repository.
