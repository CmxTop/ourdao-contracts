# Threat Model and Trust Assumptions

Status: testnet-stage, **not externally audited**. This document states what the contract assumes and what it does not defend against. It complements [`SECURITY.md`](../SECURITY.md) (reporting) and [`docs/INVARIANTS.md`](./INVARIANTS.md) (what must always hold).

## Assets

- Treasury funds held by the contract in the DAO token.
- Member stakes and accrued yield.
- Governance state: proposals, votes, loans, membership records.

## Actors and trust

| Actor | Trusted to | Not trusted to |
| --- | --- | --- |
| Admin set | Perform admin-only entrypoints (policy, pause/unpause, migration seeding) and stay online | Move member funds outside the entrypoints' rules |
| Members | Vote and act only for their own address (`require_auth` per call) | Act on another address's behalf |
| Anyone | Call permissionless entrypoints such as `mark_loan_defaulted`, which is a pure function of ledger time and loan state | Anything else |
| DAO token contract | Behave as a standard Soroban token | Nothing beyond that; a malicious token is out of scope, the DAO token is chosen at initialization |
| Off-chain backend and frontend | Index and display state | They are not a source of truth; the contract is |

## Trust assumptions

1. **Admins are honest and available.** Admin keys can pause the contract and set policy within bounds. A compromised admin quorum is the largest single risk. The consensus threshold is set at initialization.
2. **The DAO token is well behaved.** Balance-changing operations follow checks-effects-interactions, but a token that lies about balances or transfers breaks accounting.
3. **Migration seeding is trusted.** A migrated deployment relies on admins seeding state truthfully; see [`docs/MIGRATION.md`](./MIGRATION.md#trust-assumption).
4. **The contract is immutable.** There is no upgrade path, so a bug means migration to a fresh deployment.
5. **Ledger time is the only clock.** Voting periods, expiry and defaults use ledger timestamps.

## Threats considered

| Threat | Mitigation |
| --- | --- |
| Acting on another member's behalf | `require_auth()` on the member address in every state-changing entrypoint |
| Double or miscounted votes | Per-member vote tracking (`has_voted`), proposal phase guards, voting-period expiry |
| Private proposal stalling or reveal manipulation | Commit-reveal phase guards; see the open reveal-deadline and membership-fee issues below |
| Reentrancy | Soroban has no mid-call re-entry; state is still updated before token transfers |
| Rounding drift in yield | `YieldRemainder` carried forward, property tests |
| Unbounded storage / cost growth | Document hash size cap, documented membership ceiling and resource costs (#138) |

## Known gaps

- No external audit.
- Fixed membership fee and no reveal deadline on private proposals are tracked as separate issues (#47, #48).
- Testnet-only deploy tooling.
- Admin-key custody and rotation are operational concerns outside the contract.
