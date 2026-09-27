# Incident Runbook: Pausing and Unpausing the Contract

**Applies to:** `ourdao-contracts` Soroban contract  
**Entrypoints:** `pause()`, `unpause()` — both admin-only, each callable by any
single admin unilaterally (see [Decision authority](#decision-authority) below).

---

## What pause does and does not block

### Blocked while paused (value cannot leave the contract)
| Entrypoint | Why blocked |
|---|---|
| `exit_dao` | Prevents members withdrawing treasury share during a drain |
| `claim_rewards` | Prevents yield distributions |
| `unstake` | Prevents stake withdrawals |
| `disburse_approved_loan` | Prevents pending loan disbursements |
| `execute_approved` (treasury) | Prevents treasury withdrawals |
| `stake` | Prevents new commitments (state change) |
| `request_loan` | Prevents new loan proposals |
| `vote_on_loan_proposal` | Prevents governance progress |
| `propose_withdrawal` | Prevents new withdrawal proposals |
| `vote` / `commit_vote` / `reveal_vote` | Prevents governance progress |
| `edit_loan_proposal` | Prevents proposal modification |
| `mark_loan_defaulted` | Prevents slashing during pause (see loan interaction below) |
| `register_member` | Prevents new members joining |
| `register_name` | Prevents name-registry changes |
| `attach_document` | Prevents document changes |
| `expire_loan_proposal` | Prevents proposal expiration |

### **Not** blocked while paused (intentional exceptions)
| Entrypoint | Reason |
|---|---|
| `repay_loan` / `repay_loan_partial` | Borrowers must be able to repay to avoid being forced into default through no fault of their own. See [Loan interaction](#loan-term-interaction-during-a-pause) below. |
| All admin entrypoints | Admins need `unpause`, `add_admin`, `remove_admin`, `set_policy`, and `set_consensus_threshold` to remain available to manage the pause state itself. |

---

## Decision authority

Any **single admin** can call `pause()` or `unpause()` without needing consensus
from other admins. This is intentional: incident response speed matters more
than requiring multi-admin sign-off in an emergency.

The corollary is that a compromised admin key can pause the contract without
warning. Key hygiene and multi-admin redundancy are therefore essential
operational practices.

---

## The bar for pausing

Pause when you believe there is a credible, active threat to treasury funds or
member assets and the risk of **not pausing exceeds the cost of pausing**.

Concrete examples where pausing is warranted:
- An exploit is being actively executed (funds moving unexpectedly).
- A critical vulnerability has been confirmed and its trigger path is known.
- A governance attack is in progress (e.g., a single actor accumulating enough
  stake to pass proposals unilaterally).

The cost of pausing:
- Members cannot exit or claim rewards.
- Borrowers cannot receive disbursements of approved loans.
- Approved treasury withdrawals stall.

Pause speculatively only when the suspected threat is severe enough that these
consequences are acceptable.

---

## Loan-term interaction during a pause

This is a **live operational consequence** that must be understood before pausing.

Loans keep accruing toward `due_time` while the contract is paused. Because
`mark_loan_defaulted` is **also blocked while paused**, a loan can pass its
`due_time + default_grace_period` window during a pause and become instantly
markable as defaulted the moment the contract is unpaused.

**Current position:** This is an accepted trade-off. The alternative — extending
`due_time` by the pause duration — requires a state-mutating change to every
active loan on unpause, which adds complexity and a new attack surface. If you
want to change this, open an issue and describe the proposed mechanism.

**Before unpausing** after a long pause, admins should:
1. Identify all active loans and their `due_time`.
2. Check whether any loan has passed `due_time + default_grace_period` during
   the pause.
3. Coordinate with affected borrowers before unpausing, if possible, so they
   can repay immediately.

---

## Step-by-step runbook

### Pausing

1. **Identify the threat.** Confirm it is real; false alarms have a real cost
   to members.
2. **Notify other admins** (out-of-band — Telegram, Signal, Discord) that you
   are about to pause and why. This is a courtesy step, not a blocking one —
   speed matters.
3. **Call `pause()`** from an admin-authorised account. Confirm the transaction
   lands on-chain and the `paused` event is emitted.
4. **Announce to members** (Discord, forum, wherever the community is active)
   that the contract is paused, what is blocked, and that you are investigating.
   Members with pending exits or claims will want to know.
5. **Investigate** and form a plan: fix, upgrade (new deployment via
   [`docs/MIGRATION.md`](./MIGRATION.md)), or conclude the threat was a
   false alarm.

### Unpausing

1. **Verify the threat is resolved.** Get at least one other admin to
   independently agree before proceeding.
2. **Check for loans near or past their due date.** See
   [Loan-term interaction](#loan-term-interaction-during-a-pause).
3. **Run on testnet first.** Call `unpause()` on the testnet deployment, step
   through a representative set of transactions (exit, claim, vote), and confirm
   normal operation.
4. **Call `unpause()`** from an admin-authorised account. Confirm the
   `unpaused` event.
5. **Announce to members** that the contract is live again, summarise what
   happened, and what was fixed.

---

## Rehearsal

Admins should pause and unpause on testnet at least once per quarter and record
the time it takes from the decision to call `pause()` to a confirmed on-chain
transaction. Target: under 5 minutes.

Log the date and duration in the team's ops notes.

---

## References

- `contracts/dao/src/admin.rs:126–146` — `pause` and `unpause` implementations
- `contracts/dao/src/util.rs:35–41` — `require_not_paused`
- `contracts/dao/src/loans.rs:413–425` — pause guard in loan flow
- [`PAUSE_FUNCTIONALITY.md`](PAUSE_FUNCTIONALITY.md) — full list of guarded entrypoints
- [`SECURITY.md`](../SECURITY.md) — how to report a vulnerability
