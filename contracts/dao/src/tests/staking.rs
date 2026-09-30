use soroban_sdk::testutils::Events as _;
use soroban_sdk::xdr::{ContractEventBody, ScVal};

use super::common::*;
use crate::types::ProposalStatus;
use crate::Error;

#[test]
fn staking_boosts_voting_weight() {
    let s = setup(2); // required for loan = ceil(2*51%) = 2
    let borrower = s.members.get(0).unwrap();
    let staker = s.members.get(1).unwrap();

    // Stake enough for +2 weight (200 / 100). One staked yes-vote = weight 3 >= 2.
    s.client.stake(&staker, &200);
    assert_eq!(s.client.get_stake(&staker), 200);

    let pid = s.client.request_loan(&borrower, &500, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&staker, &pid, &true);

    let prop = s.client.get_loan_proposal(&pid).unwrap();
    assert_eq!(prop.for_votes, 3);
    assert_eq!(prop.status, ProposalStatus::Approved);

    // Unstake returns tokens.
    let before = s.token.balance(&staker);
    s.client.unstake(&staker, &200);
    assert_eq!(s.token.balance(&staker), before + 200);
    assert_eq!(s.client.get_stake(&staker), 0);
}

#[test]
fn rejected_stake_transfer_leaves_stake_storage_unchanged() {
    let s = rejecting_setup(1);
    let member = s.members.get(0).unwrap();
    let dao_balance_before = s.token.balance(&s.client.address);

    s.token.set_reject_transfers(&true);
    let result = s.client.try_stake(&member, &500);
    assert!(result.is_err());

    assert_eq!(s.client.get_stake(&member), 0);
    let total_staked = s
        .env
        .as_contract(&s.client.address, || crate::storage::get_total_staked(&s.env));
    assert_eq!(total_staked, 0);
    assert_eq!(s.token.balance(&s.client.address), dao_balance_before);
    let has_stake_time = s.env.as_contract(&s.client.address, || {
        s.env
            .storage()
            .persistent()
            .has(&crate::storage::DataKey::StakeTime(member.clone()))
    });
    assert!(
        !has_stake_time,
        "stake timestamp must roll back with the rejected transfer"
    );
}

// Issue #193: StakingRewardClaimed event on yield distribution
#[test]
fn claim_rewards_emits_staking_reward_claimed_event_and_updates_snapshot() {
    let s = setup(3);
    let borrower = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();

    let pid = s.client.request_loan(&borrower, &1_000, &None);
    advance(&s.env, EDITING + 1);
    s.client.vote_on_loan_proposal(&v1, &pid, &true);
    s.client.vote_on_loan_proposal(&v2, &pid, &true);
    s.client.repay_loan(&borrower, &0);

    let loan = s.client.get_loan(&0).unwrap();
    let interest = loan.total_repayment - loan.principal;
    let expected_share = interest / 3;
    assert!(expected_share > 0);

    assert_eq!(s.client.get_pending_yield(&v1), expected_share);

    // Claim rewards
    let claimed = s.client.claim_rewards(&v1);
    assert_eq!(claimed, expected_share);

    // Verify event payload and topics
    assert!(emitted(&s.env, "claimed"));

    // Find the claimed event in event log
    let all_events = s.env.events().all();
    let events_vec = all_events.events();
    let event = events_vec
        .iter()
        .rev()
        .find(|e| {
            let ContractEventBody::V0(body) = &e.body;
            matches!(body.topics.first(), Some(ScVal::Symbol(sym)) if sym.0.to_utf8_string_lossy() == "claimed")
        })
        .expect("claimed event not found");

    let ContractEventBody::V0(body) = &event.body;
    assert_eq!(body.topics.len(), 3);
    // Topic 0: symbol "claimed"
    match &body.topics[0] {
        ScVal::Symbol(sym) => assert_eq!(sym.0.to_utf8_string_lossy(), "claimed"),
        _ => panic!("unexpected topic 0"),
    }
    // Verify topic 2 has the claimed amount
    match &body.topics[2] {
        ScVal::I128(amount) => {
            let val = ((amount.hi as i128) << 64) | (amount.lo as i128);
            assert_eq!(val, expected_share);
        }
        _ => panic!("unexpected topic 2"),
    }

    // Verify accumulator snapshot updated on member record
    assert_eq!(s.client.get_pending_yield(&v1), 0);
    assert_eq!(s.client.try_claim_rewards(&v1), Err(Ok(Error::NothingToClaim)));
}
