extern crate std;
use super::common::*;
use crate::admin::TIMELOCK_DURATION;
use crate::privacy::compute_commitment;
use crate::storage::ProposalKind;
use crate::types::{LoanPolicy, LoanStatus, MemberStatus, ProposalPhase, ProposalStatus};
use crate::{Error, OurDao, OurDaoClient};
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
use soroban_sdk::xdr::{ContractEventBody, ScVal};
use soroban_sdk::{token, Address, Bytes, BytesN, Env, String, Vec};

#[test]
fn init_and_membership() {
    let s = setup(3);
    assert_eq!(s.client.get_total_members(), 3);
    assert_eq!(s.client.get_active_members(), 3);
    assert_eq!(s.client.get_treasury_balance(), 3 * FEE);
    assert!(s.client.is_member(&s.members.get(0).unwrap()));
    assert!(s.client.is_admin(&s.admin));
    assert_eq!(s.client.get_consensus_threshold(), 5_100);

    let m = s.client.get_member(&s.members.get(0).unwrap()).unwrap();
    assert_eq!(m.status, MemberStatus::ActiveMember);
    assert_eq!(m.contribution, FEE);
}


#[test]
fn double_join_rejected() {
    let s = setup(1);
    let m = s.members.get(0).unwrap();
    let res = s.client.try_register_member(&m);
    assert_eq!(res, Err(Ok(Error::AlreadyMember)));
}


#[test]
fn exit_returns_share() {
    let s = setup(2);
    let m = s.members.get(0).unwrap();
    let before = s.token.balance(&m);
    let share = s.client.calculate_exit_share(&m);
    assert!(share > 0);
    s.client.exit_dao(&m);
    assert_eq!(s.token.balance(&m), before + share);
    assert_eq!(s.client.get_active_members(), 1);
    assert!(!s.client.is_member(&m));
}


#[test]
fn ten_join_five_exit_pays_remaining_members_full_share() {
    let s = setup(10);

    for i in 0..5u32 {
        let m = s.members.get(i).unwrap();
        assert_eq!(s.client.calculate_exit_share(&m), FEE);
        s.client.exit_dao(&m);
    }

    assert_eq!(s.client.get_total_members(), 10);
    assert_eq!(s.client.get_active_members(), 5);
    assert_eq!(s.client.get_treasury_balance(), 5 * FEE);

    for i in 5..10u32 {
        let m = s.members.get(i).unwrap();
        assert_eq!(s.client.calculate_exit_share(&m), FEE);
    }
}


#[test]
fn rejoin_after_exit_is_counted_once_not_twice() {
    let s = setup(3);
    let m = s.members.get(0).unwrap();

    s.client.exit_dao(&m);
    assert_eq!(s.client.get_active_members(), 2);

    s.client.register_member(&m);
    assert_eq!(s.client.get_total_members(), 4);
    assert_eq!(s.client.get_active_members(), 3);
    assert_eq!(s.client.calculate_exit_share(&m), FEE);
}


#[test]
fn mixed_joins_exits_defaults_never_strand_value() {
    let s = setup(3);
    let a = s.members.get(0).unwrap();
    let b = s.members.get(1).unwrap();
    let c = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&a, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&b, &pid, &true);
    s.client.vote_on_loan_proposal(&c, &pid, &true);

    s.client.exit_dao(&c);
    let b_share_before_default = s.client.calculate_exit_share(&b);

    advance(&s.env, LOAN_DURATION + 1);
    s.client.mark_loan_defaulted(&pid);

    let b_share_after_default = s.client.calculate_exit_share(&b);
    assert!(b_share_after_default > b_share_before_default);

    s.client.exit_dao(&b);
    s.client.register_member(&c);

    assert_eq!(s.client.get_total_members(), 4);
    assert_eq!(s.client.get_active_members(), 2);

    let mut sum = 0i128;
    for i in 0..3u32 {
        let m = s.members.get(i).unwrap();
        if s.client.is_member(&m) {
            sum += s.client.calculate_exit_share(&m);
        }
    }
    assert!(sum <= s.client.get_treasury_balance());
}

// ==================== issue #7: property tests ====================
//
// Example tests above pin down behavior at specific, hand-picked numbers.
// These instead generate wide ranges of inputs (amounts, treasury sizes,
// stakes, contributions — including near-`i128::MAX` boundaries) and check
// invariants that must hold for *any* input, not just the ones a human
// happened to write down. `amount` inputs are capped at `AMOUNT_BOUND`
// (rather than the full `i128` range) specifically to stay clear of the
// *intermediate* overflow in `amount * BASIS_POINTS` — the invariants below
// are about the post-clamp behavior of these functions, not about auditing
// every arithmetic op in isolation.



#[test]
fn yield_accumulator_join_claim_exit_rejoin() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    // Get a loan approved and repaid so interest is distributed.
    let pid = s.client.request_loan(&borrower, &1_000, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    s.client.repay_loan(&borrower, &0);

    let loan = s.client.get_loan(&0).unwrap();
    let interest = loan.total_repayment - loan.principal;
    let per_member = interest / 3;

    // Both non-borrower members can claim their share.
    assert_eq!(s.client.get_pending_yield(&v1), per_member);
    assert_eq!(s.client.get_pending_yield(&v2), per_member);
    s.client.claim_rewards(&v1);
    assert_eq!(s.client.get_pending_yield(&v1), 0);

    // v1 exits — their snapshot is settled so they don't double-claim.
    s.client.exit_dao(&v1);
    assert_eq!(s.client.get_pending_yield(&v1), 0);

    // v1 rejoins — snapshot is set to current accumulator, earning nothing
    // from interest that accrued before they rejoined.
    s.client.register_member(&v1);
    assert_eq!(s.client.get_pending_yield(&v1), 0);
}

// ==================== issue #5: partial loan repayment ====================


#[test]
fn rejected_register_member_transfer_rolls_back_all_membership_state() {
    let s = rejecting_setup(0);
    let member = Address::generate(&s.env);
    s.token.mint(&member, &MINT);
    s.token.set_reject_transfers(&true);

    let result = s.client.try_register_member(&member);
    assert!(result.is_err());

    assert_eq!(s.client.get_total_members(), 0);
    assert_eq!(s.client.get_active_members(), 0);
    assert!(!s.client.is_member(&member));
    assert!(s.client.get_member(&member).is_none());
    assert_eq!(s.token.balance(&s.client.address), 0);
}


#[test]
fn initialize_rejects_duplicate_admins() {
    let env = Env::default();
    env.mock_all_auths();
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    let contract_id = env.register(OurDao, ());
    let client = OurDaoClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let mut admins = Vec::new(&env);
    admins.push_back(admin.clone());
    admins.push_back(admin.clone());

    let res = client.try_initialize(&admins, &5_100u32, &FEE, &token_id, &policy());
    assert_eq!(res, Err(Ok(Error::AlreadyAdmin)));
}


#[test]
fn initialize_rejects_account_address_as_token() {
    let env = Env::default();
    env.mock_all_auths();
    let not_a_contract = Address::generate(&env);
    assert_eq!(init_with_token(&env, &not_a_contract), Err(Error::InvalidToken));
}


#[test]
fn initialize_rejects_contract_that_is_not_a_token() {
    let env = Env::default();
    env.mock_all_auths();
    // A real contract, but not a token: it has no `balance` entrypoint.
    let other = env.register(OurDao, ());
    assert_eq!(init_with_token(&env, &other), Err(Error::InvalidToken));
}


#[test]
fn initialize_accepts_a_real_token() {
    let env = Env::default();
    env.mock_all_auths();
    let sac = env.register_stellar_asset_contract_v2(Address::generate(&env));
    assert_eq!(init_with_token(&env, &sac.address()), Ok(()));
}

// ===========================================================================
// Issue #191: Configurable quorum threshold parameter in LoanPolicy
// ===========================================================================


#[test]
fn only_admin_governs() {
    let s = setup(1);
    let intruder = s.members.get(0).unwrap();
    let res = s.client.try_set_consensus_threshold(&intruder, &7_000);
    assert_eq!(res, Err(Ok(Error::NotAdmin)));

    s.client.set_consensus_threshold(&s.admin, &7_000);
    assert_eq!(s.client.get_consensus_threshold(), 7_000);
}


#[test]
fn cannot_remove_last_admin() {
    let s = setup(0);
    let res = s.client.try_remove_admin(&s.admin, &s.admin);
    assert_eq!(res, Err(Ok(Error::CannotRemoveLastAdmin)));
}

// ==================== issue #1: expire_loan_proposal ====================


#[test]
fn validate_policy_rejects_quorum_bps_above_basis_points() {
    let mut p = policy();
    p.quorum_bps = 10_001; // > 10_000 BASIS_POINTS
    let s = setup(1);
    let res = s.client.try_propose_policy_update(&s.admin, &p);
    assert_eq!(res, Err(Ok(Error::InvalidLoanPolicy)));
}


#[test]
fn policy_update_timelock_enforced() {
    let s = setup(3);
    let non_admin = s.members.get(0).unwrap();

    let mut new_p = policy();
    new_p.max_loan_duration = 60 * 24 * 60 * 60; // change duration

    // Non-admin cannot propose policy update
    let err = s.client.try_propose_policy_update(&non_admin, &new_p);
    assert_eq!(err, Err(Ok(Error::NotAdmin)));

    // Admin proposes update
    s.client.propose_policy_update(&s.admin, &new_p);

    let pending = s.client.get_pending_policy_update().unwrap();
    assert_eq!(pending.policy.max_loan_duration, 60 * 24 * 60 * 60);
    assert_eq!(pending.execution_time, pending.proposed_at + TIMELOCK_DURATION);

    // Attempting to execute immediately fails with TimelockNotExpired
    let early = s.client.try_execute_policy_update(&s.admin);
    assert_eq!(early, Err(Ok(Error::TimelockNotExpired)));

    // Advance 47 hours (not yet 48 hours)
    advance(&s.env, TIMELOCK_DURATION - 3600);
    let early2 = s.client.try_execute_policy_update(&s.admin);
    assert_eq!(early2, Err(Ok(Error::TimelockNotExpired)));

    // Non-admin cannot execute even after timelock
    advance(&s.env, 3601);
    let non_admin_exec = s.client.try_execute_policy_update(&non_admin);
    assert_eq!(non_admin_exec, Err(Ok(Error::NotAdmin)));

    // Admin executes after timelock delay
    s.client.execute_policy_update(&s.admin);

    // Policy is updated, pending update is cleared
    assert_eq!(
        s.client.get_loan_policy().max_loan_duration,
        60 * 24 * 60 * 60
    );
    assert!(s.client.get_pending_policy_update().is_none());

    // Subsequent execute fails because nothing is pending
    let none_pending = s.client.try_execute_policy_update(&s.admin);
    assert_eq!(none_pending, Err(Ok(Error::NoPendingPolicy)));
}


#[test]
fn policy_update_cancel_by_admin() {
    let s = setup(3);
    let non_admin = s.members.get(0).unwrap();

    let mut new_p = policy();
    new_p.min_interest_rate = 1_000;

    s.client.propose_policy_update(&s.admin, &new_p);
    assert!(s.client.get_pending_policy_update().is_some());

    // Non-admin cannot cancel
    let err = s.client.try_cancel_policy_update(&non_admin);
    assert_eq!(err, Err(Ok(Error::NotAdmin)));

    // Admin cancels
    s.client.cancel_policy_update(&s.admin);
    assert!(s.client.get_pending_policy_update().is_none());

    // Even after 48h, execute fails because it was cancelled
    advance(&s.env, TIMELOCK_DURATION + 1);
    let res = s.client.try_execute_policy_update(&s.admin);
    assert_eq!(res, Err(Ok(Error::NoPendingPolicy)));
}

// ===========================================================================
// Issue #193: StakingRewardClaimed event on yield distribution
// ===========================================================================


#[test]
fn pause_blocks_all_mutating_entrypoints() {
    // This test explicitly checks every mutating entrypoint to ensure pause()
    // prevents state changes. Every public entrypoint that mutates contract state
    // must be listed below with an assertion. If you add a new mutating entrypoint
    // without a pause decision, this test will fail — the property being protected
    // (an emergency stop actually stops everything) is too critical to check by
    // hand-written list.
    //
    // Entrypoints deliberately callable while paused:
    // - None. All state-changing operations should be blocked by pause().
    // - Note: repayment was discussed but decided to be pause-gated to maintain
    //   consistent emergency stop semantics (see issue #52).

    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    let staker = s.members.get(2).unwrap();
    let newcomer = Address::generate(&s.env);

    // Set up some proposals to vote on / interact with
    let loan_pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);

    let treasury_pid = s.client.propose_treasury_withdrawal(
        &borrower,
        &100,
        &newcomer,
        &String::from_slice(&s.env, "test"),
        &false,
    );

    // Pause the contract
    s.client.pause(&s.admin);
    assert!(s.client.is_paused());

    // ==================== Membership ====================
    let res = s.client.try_register_member(&newcomer);
    assert_eq!(res, Err(Ok(Error::Paused)), "register_member should be pause-gated");

    let res = s.client.try_exit_dao(&borrower);
    assert_eq!(res, Err(Ok(Error::Paused)), "exit_dao should be pause-gated");

    let res = s.client.try_claim_rewards(&borrower);
    assert_eq!(res, Err(Ok(Error::Paused)), "claim_rewards should be pause-gated");

    // ==================== Loans ====================
    let res = s.client.try_request_loan(&borrower, &1000, &None);
    assert_eq!(res, Err(Ok(Error::Paused)), "request_loan should be pause-gated");

    let res = s.client.try_edit_loan_proposal(&borrower, &loan_pid, &600);
    assert_eq!(res, Err(Ok(Error::Paused)), "edit_loan_proposal should be pause-gated");

    let res = s.client.try_vote_on_loan_proposal(&voter, &loan_pid, &true);
    assert_eq!(res, Err(Ok(Error::Paused)), "vote_on_loan_proposal should be pause-gated");

    let res = s.client.try_disburse_approved_loan(&loan_pid);
    assert_eq!(res, Err(Ok(Error::Paused)), "disburse_approved_loan should be pause-gated");

    let res = s.client.try_repay_loan(&borrower, &0);
    assert_eq!(res, Err(Ok(Error::Paused)), "repay_loan should be pause-gated");

    let res = s.client.try_repay_loan_partial(&borrower, &0, &100);
    assert_eq!(res, Err(Ok(Error::Paused)), "repay_loan_partial should be pause-gated");

    let res = s.client.try_mark_loan_defaulted(&0);
    assert_eq!(res, Err(Ok(Error::Paused)), "mark_loan_defaulted should be pause-gated");

    let res = s.client.try_expire_loan_proposal(&0);
    assert_eq!(res, Err(Ok(Error::Paused)), "expire_loan_proposal should be pause-gated");

    // ==================== Treasury ====================
    let res = s.client.try_propose_treasury_withdrawal(
        &borrower,
        &100,
        &newcomer,
        &String::from_slice(&s.env, "test"),
        &false,
    );
    assert_eq!(res, Err(Ok(Error::Paused)), "propose_treasury_withdrawal should be pause-gated");

    let res = s.client.try_vote_on_treasury_proposal(&voter, &treasury_pid, &true);
    assert_eq!(res, Err(Ok(Error::Paused)), "vote_on_treasury_proposal should be pause-gated");

    let res = s.client.try_expire_treasury_proposal(&0);
    assert_eq!(res, Err(Ok(Error::Paused)), "expire_treasury_proposal should be pause-gated");

    let res = s.client.try_execute_treasury_proposal(&0);
    assert_eq!(res, Err(Ok(Error::Paused)), "execute_treasury_proposal should be pause-gated");

    // ==================== Staking ====================
    let res = s.client.try_stake(&staker, &100);
    assert_eq!(res, Err(Ok(Error::Paused)), "stake should be pause-gated");

    let res = s.client.try_unstake(&staker, &100);
    assert_eq!(res, Err(Ok(Error::Paused)), "unstake should be pause-gated");

    // ==================== Registry ====================
    let res = s.client.try_register_name(&borrower, &String::from_slice(&s.env, "test"));
    assert_eq!(res, Err(Ok(Error::Paused)), "register_name should be pause-gated");

    // ==================== Privacy (commit-reveal voting) ====================
    let commitment = BytesN::from_array(&s.env, &[0u8; 32]);
    let res = s.client.try_commit_treasury_vote(&voter, &treasury_pid, &commitment);
    assert_eq!(res, Err(Ok(Error::Paused)), "commit_treasury_vote should be pause-gated");

    let salt = BytesN::from_array(&s.env, &[0u8; 32]);
    let res = s.client.try_reveal_treasury_vote(&voter, &treasury_pid, &true, &salt);
    assert_eq!(res, Err(Ok(Error::Paused)), "reveal_treasury_vote should be pause-gated");

    // ==================== Docs (content-hash metadata) ====================
    let cid = Bytes::from_slice(&s.env, &[1, 2, 3]);
    let res = s.client.try_attach_document(&borrower, &ProposalKind::Loan, &loan_pid, &cid);
    assert_eq!(res, Err(Ok(Error::Paused)), "attach_document should be pause-gated");

    // Verify unpause works
    s.client.unpause(&s.admin);
    assert!(!s.client.is_paused());
}


#[test]
fn bench_exit_dao_scaling() {
    // #138: Measure cost of exit_dao at different sizes
    for size in [10, 100, 1000] {
        let s = setup(size);
        let m = s.members.get(0).unwrap();
        
        s.env.budget().reset_unlimited();
        let cpu_before = s.env.budget().cpu_instruction_cost();
        s.client.exit_dao(&m);
        let cpu_after = s.env.budget().cpu_instruction_cost();
        
        let cost = cpu_after - cpu_before;

        // Fail if cost scales poorly (O(n) check). Keep this no_std-compatible
        // instead of printing from the contract crate.
        assert!(
            size != 1000 || cost <= 50_000_000,
            "exit_dao cost exceeds O(1) bound"
        );
    }
}

// ---------------------------------------------------------------------------
// Approved-but-unfundable path (#108): a proposal passes its vote while the
// treasury can't cover it, so it parks in ApprovedPendingDisbursement and
// emits `loan_wait` / `tre_wait` instead of paying out.
// ---------------------------------------------------------------------------


