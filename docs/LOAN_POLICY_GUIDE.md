# LoanPolicy Parameter Guide

`LoanPolicy` has twelve tunable parameters that control who can borrow, how
much, at what rate, and what happens when a loan goes wrong. This document
explains each parameter: what it controls, its units, its interactions with
other parameters, and a suggested starting value.

The final section provides a worked example for a small DAO and calls out
pathological combinations explicitly.

---

## Parameter reference

### `min_membership_duration: u64`
**Units:** seconds since Unix epoch (note: stored as `join_ledger` — a timestamp,
not a ledger sequence number; see [#55]).  
**What it controls:** How long a member must have been in the DAO before
they can request a loan.  
**Interactions:** Read against `member.join_ledger` at loan-request time.
Setting this above the typical membership age of your DAO members will make
lending impossible in practice until enough time has passed.  
**Suggested start:** `2_592_000` (30 days). Long enough to filter drive-by
borrowers; short enough not to strand early members.

---

### `membership_contribution: i128`
**Units:** token stroops (the DAO's native token, smallest unit).  
**What it controls:** The minimum contribution a member must have on record
to be eligible for a loan. Set this to the membership fee to require that a
member has not been slashed below the threshold.  
**Interactions:** Read against `member.contribution` at loan-request time.
If a member's contribution has been reduced by a default penalty, they may
fall below this threshold.  
**Suggested start:** equal to the membership fee (e.g., `1_000`).

---

### `max_loan_duration: u64`
**Units:** seconds.  
**What it controls:** The maximum repayment term a borrower can request.  
**Interactions:** Combined with `default_grace_period`, this sets the outer
boundary on how long a defaulted loan can sit before it becomes markable.
A very long `max_loan_duration` plus a long `grace_period` can leave bad
debt open for years.  
**Suggested start:** `2_592_000` (30 days).  
**Validation:** Must be non-zero. `validate_policy` does not cap this, so
choose carefully.

---

### `min_interest_rate: u32` and `max_interest_rate: u32`
**Units:** basis points (1 bp = 0.01%; 10_000 bp = 100%).  
**What they control:** The linear interest-rate curve. A small loan relative
to the treasury gets `min_interest_rate`; a loan at the maximum ratio gets
`max_interest_rate`. Any amount in between is interpolated linearly and
clamped to this range.  
**Interactions:** The property test `loan_terms_rate_stays_within_policy_bounds`
verifies that the quoted rate is always within `[min, max]`.  
**Constraint:** `min_interest_rate <= max_interest_rate`; both must be ≤
10_000 (100%).  
**Suggested start:** `min = 500` (5%), `max = 2_000` (20%).

---

### `cooldown_period: u64`
**Units:** seconds.  
**What it controls:** How long a borrower must wait after their last loan
before requesting another. Read against `member.last_loan_time`.  
**Interactions:** `last_loan_time` is a Unix timestamp. A `cooldown_period`
of 0 allows back-to-back borrowing.  
**Suggested start:** `2_592_000` (30 days). Prevents serial borrowers from
monopolising treasury liquidity.

---

### `max_loan_to_treasury_ratio: u32`
**Units:** basis points of the treasury balance at loan-request time.  
**What it controls:** The ceiling on any single loan as a fraction of the
live treasury.  
**Pathological value — `0`:** A ratio of 0 makes **every loan impossible**
while still passing `validate_policy` (see [#53]). Do not set this to 0.  
**Suggested start:** `2_000` (20%). Limits exposure to any single borrower.

---

### `default_grace_period: u64`
**Units:** seconds.  
**What it controls:** Extra time after `Loan.due_time` before an admin can
call `mark_loan_defaulted`. This gives a late borrower a window to repay
without being defaulted.  
**Interactions:** During a pause, `mark_loan_defaulted` is blocked, so a
loan can silently pass `due_time + grace_period` and become instantly
markable on unpause. See [INCIDENT_RUNBOOK.md](./INCIDENT_RUNBOOK.md).  
**Suggested start:** `259_200` (3 days).

---

### `default_penalty_bps: u32`
**Units:** basis points of `member.contribution`.  
**What it controls:** How much of a defaulting borrower's on-record contribution
is slashed on default. This amount is burned (removed from the treasury's
accounting of their stake), not redistributed.  
**Interactions:** Slashing can reduce `member.contribution` below
`membership_contribution`, making the member ineligible for future loans.  
**Suggested start:** `2_000` (20%).  
**Constraint:** Must be ≤ 10_000 (100%).

---

### `editing_period: u64`
**Units:** seconds.  
**What it controls:** How long a loan proposal stays in the `Editing` phase
before it advances to `Voting`. The borrower can edit terms during this window.  
**Interactions:** `editing_period + voting_period` determines the total elapsed
time from proposal to decision. Capital is effectively committed during this
window because `disburse_approved_loan` can only be called after voting closes.  
**Constraint:** Must be non-zero and ≤ `30 * 24 * 60 * 60` (30 days).  
**Suggested start:** `259_200` (3 days).

---

### `voting_period: u64`
**Units:** seconds.  
**What it controls:** How long voting is open on a proposal.  
**Interactions:** See `editing_period` above.  
**Constraint:** Must be non-zero and ≤ 30 days.  
**Suggested start:** `259_200` (3 days).

---

### `treasury_threshold: u32`
**Units:** basis points of the total weighted vote.  
**What it controls:** The approval threshold for treasury-withdrawal proposals
(not loan proposals — those use `consensus_threshold`).  
**Constraint:** Must be non-zero and ≤ 10_000 (100%).  
**Suggested start:** `5_100` (51%).

---

## Pathological combinations

| Combination | Effect |
|---|---|
| `max_loan_to_treasury_ratio = 0` | Every loan request fails with `InvalidLoanPolicy` — effectively no lending. Passes `validate_policy`. See [#53]. |
| `editing_period + voting_period` > members' patience | Proposals expire before gathering quorum; effective lending rate approaches zero. |
| `max_loan_duration` >> `default_grace_period` | A defaulted loan can sit in limbo for the full loan duration before anyone can act. |
| `min_membership_duration` > age of the DAO | No member is old enough to borrow. |
| `cooldown_period` > `max_loan_duration` | A borrower cannot request a second loan until the cooldown clears, even if they repaid early. Usually fine; document if intentional. |

Which parameters are effectively permanent once obligations exist: any parameter
that is read live against open loans (`max_loan_duration`, `min_interest_rate`,
`max_interest_rate`, `default_grace_period`, `default_penalty_bps`) is safe to
change for future loans but affects how existing open loans appear relative to
the new policy. A retroactive-policy issue is tracked separately in this repo.

---

## Worked example: small DAO (5–20 members)

```rust
LoanPolicy {
    min_membership_duration: 2_592_000,   // 30 days — filters drive-by joiners
    membership_contribution: 1_000,       // equal to the membership fee
    max_loan_duration: 2_592_000,         // 30 days
    min_interest_rate: 500,               // 5%  — floor for any loan
    max_interest_rate: 2_000,             // 20% — ceiling for max-ratio loans
    cooldown_period: 2_592_000,           // 30 days — one loan per month max
    max_loan_to_treasury_ratio: 2_000,    // 20% of treasury per loan
    default_grace_period: 259_200,        // 3 days past due_time
    default_penalty_bps: 2_000,           // 20% contribution slashed on default
    editing_period: 259_200,              // 3 days editing
    voting_period: 259_200,               // 3 days voting
    treasury_threshold: 5_100,            // 51% approval for treasury withdrawals
}
```

**Reasoning:**
- 30-day membership minimum and cooldown protect against hit-and-run borrowers.
- 20% treasury ratio means a single loan can't empty the DAO, but is large
  enough to be useful.
- 5–20% rate range rewards the DAO fairly while staying below predatory rates.
- 3-day editing + voting keeps decisions moving without rushing members.
- 3-day grace period gives honest borrowers a short buffer without leaving bad
  debt open for long.
- 51% threshold for treasury withdrawals matches the consensus threshold,
  keeping governance consistent.

---

## References

- `contracts/dao/src/types.rs:58–76` — `LoanPolicy` struct definition
- `contracts/dao/src/admin.rs:8–26` — `validate_policy`
- `scripts/deploy-testnet.sh` — deploy script (add a default policy here)
- Open issues: [#53](https://github.com/ourdao/ourdao-contracts/issues/53),
  [#54](https://github.com/ourdao/ourdao-contracts/issues/54),
  [#55](https://github.com/ourdao/ourdao-contracts/issues/55)
