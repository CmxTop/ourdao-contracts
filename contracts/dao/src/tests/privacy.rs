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
fn commit_reveal_private_treasury_vote() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let v2 = s.members.get(2).unwrap();
    let dest = Address::generate(&s.env);

    let reason = String::from_str(&s.env, "secret grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &600, &dest, &reason, &true);

    // Open voting is refused on a private proposal.
    let open = s.client.try_vote_on_treasury_proposal(&v1, &pid, &true);
    assert_eq!(open, Err(Ok(Error::NotAuthorized)));

    let salt1 = BytesN::from_array(&s.env, &[7u8; 32]);
    let salt2 = BytesN::from_array(&s.env, &[9u8; 32]);
    let c1 = compute_commitment(&s.env, true, &salt1);
    let c2 = compute_commitment(&s.env, true, &salt2);

    s.client.commit_treasury_vote(&v1, &pid, &c1);
    s.client.commit_treasury_vote(&v2, &pid, &c2);

    // A reveal that doesn't match the commitment is rejected.
    let bad = s.client.try_reveal_treasury_vote(&v1, &pid, &false, &salt1);
    assert_eq!(bad, Err(Ok(Error::CommitmentMismatch)));

    s.client.reveal_treasury_vote(&v1, &pid, &true, &salt1);
    assert_eq!(
        s.client.get_treasury_proposal(&pid).unwrap().status,
        ProposalStatus::Pending
    );
    s.client.reveal_treasury_vote(&v2, &pid, &true, &salt2);
    assert_eq!(
        s.client.get_treasury_proposal(&pid).unwrap().status,
        ProposalStatus::Executed
    );
    assert_eq!(s.token.balance(&dest), 600);
}


#[test]
fn commit_vote_cannot_be_overwritten() {
    let s = setup(2);
    let proposer = s.members.get(0).unwrap();
    let voter = s.members.get(1).unwrap();
    let dest = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "secret grant");
    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &600, &dest, &reason, &true);

    let salt = BytesN::from_array(&s.env, &[7u8; 32]);
    let other = BytesN::from_array(&s.env, &[9u8; 32]);
    let c1 = compute_commitment(&s.env, true, &salt);
    let c2 = compute_commitment(&s.env, false, &other);

    s.client.commit_treasury_vote(&voter, &pid, &c1);
    let again = s.client.try_commit_treasury_vote(&voter, &pid, &c2);
    assert_eq!(again, Err(Ok(Error::AlreadyVoted)));

    // The original commitment is still the one that reveals successfully.
    advance(&s.env, VOTING_PERIOD + 1);
    s.client.reveal_treasury_vote(&voter, &pid, &true, &salt);
}


#[test]
fn has_voted_treasury_commit_reveal() {
    let s = setup(3);
    let proposer = s.members.get(0).unwrap();
    let v1 = s.members.get(1).unwrap();
    let dest = Address::generate(&s.env);
    let reason = String::from_str(&s.env, "grant");

    let pid = s
        .client
        .propose_treasury_withdrawal(&proposer, &100, &dest, &reason, &true);

    // Before commit: not voted.
    assert!(!s.client.has_voted(&ProposalKind::Treasury, &pid, &v1));

    // After commit: voted (commitment counts as a vote for dedup).
    let salt = BytesN::from_array(&s.env, &[1u8; 32]);
    let commitment = compute_commitment(&s.env, true, &salt);
    s.client.commit_treasury_vote(&v1, &pid, &commitment);
    assert!(s.client.has_voted(&ProposalKind::Treasury, &pid, &v1));
}

// ==================== issue #3: name validation ====================


