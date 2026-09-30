extern crate std;
use std::println;

use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
use soroban_sdk::xdr::{ContractEventBody, ScVal};
use soroban_sdk::{token, Address, Bytes, BytesN, Env, String, Vec};

use crate::privacy::compute_commitment;
use crate::storage::ProposalKind;
use crate::admin::TIMELOCK_DURATION;
use crate::types::{LoanPolicy, LoanStatus, MemberStatus, ProposalPhase, ProposalStatus};
use crate::{Error, OurDao, OurDaoClient};

pub(crate) const FEE: i128 = 1_000;
pub(crate) const MINT: i128 = 1_000_000;
pub(crate) const EDITING: u64 = 3 * 24 * 60 * 60;
pub(crate) const VOTING_PERIOD: u64 = 3 * 24 * 60 * 60;
pub(crate) const LOAN_DURATION: u64 = 30 * 24 * 60 * 60;

#[soroban_sdk::contracttype]
#[derive(Clone)]
enum RejectingTokenKey {
    Balance(Address),
    RejectTransfers,
}

#[soroban_sdk::contract]
pub struct RejectingToken;

#[soroban_sdk::contractimpl]
impl RejectingToken {
    pub fn mint(env: Env, to: Address, amount: i128) {
        let key = RejectingTokenKey::Balance(to);
        let current: i128 = env.storage().instance().get(&key).unwrap_or(0);
        env.storage().instance().set(&key, &(current + amount));
    }

    pub fn set_reject_transfers(env: Env, reject: bool) {
        env.storage()
            .instance()
            .set(&RejectingTokenKey::RejectTransfers, &reject);
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage()
            .instance()
            .get(&RejectingTokenKey::Balance(id))
            .unwrap_or(0)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        let reject: bool = env
            .storage()
            .instance()
            .get(&RejectingTokenKey::RejectTransfers)
            .unwrap_or(false);
        if reject {
            panic!("mock token transfer rejected");
        }
        if amount < 0 {
            panic!("negative transfer");
        }

        let from_key = RejectingTokenKey::Balance(from);
        let to_key = RejectingTokenKey::Balance(to);
        let from_balance: i128 = env.storage().instance().get(&from_key).unwrap_or(0);
        if from_balance < amount {
            panic!("insufficient balance");
        }
        let to_balance: i128 = env.storage().instance().get(&to_key).unwrap_or(0);
        env.storage()
            .instance()
            .set(&from_key, &(from_balance - amount));
        env.storage().instance().set(&to_key, &(to_balance + amount));
    }
}

pub(crate) struct RejectingSetup<'a> {
    pub(crate) env: Env,
    pub(crate) client: OurDaoClient<'a>,
    pub(crate) token: RejectingTokenClient<'a>,
    pub(crate) members: Vec<Address>,
}

pub(crate) fn rejecting_setup(num_members: u32) -> RejectingSetup<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let token_id = env.register(RejectingToken, ());
    let token = RejectingTokenClient::new(&env, &token_id);
    let admin = Address::generate(&env);
    let contract_id = env.register(OurDao, ());
    let client = OurDaoClient::new(&env, &contract_id);

    let mut admins = Vec::new(&env);
    admins.push_back(admin);
    client.initialize(&admins, &5_100u32, &FEE, &token_id, &policy());

    let mut members = Vec::new(&env);
    for _ in 0..num_members {
        let member = Address::generate(&env);
        token.mint(&member, &MINT);
        client.register_member(&member);
        members.push_back(member);
    }

    RejectingSetup {
        env,
        client,
        token,
        members,
    }
}

pub(crate) struct Setup<'a> {
    pub(crate) env: Env,
    pub(crate) client: OurDaoClient<'a>,
    pub(crate) token: token::Client<'a>,
    pub(crate) admin: Address,
    pub(crate) members: Vec<Address>,
}

pub(crate) fn policy() -> LoanPolicy {
    LoanPolicy {
        min_membership_duration: 0,
        membership_contribution: FEE,
        max_loan_duration: 30 * 24 * 60 * 60,
        min_interest_rate: 500,   // 5%
        max_interest_rate: 2_000, // 20%
        cooldown_period: 0,
        max_loan_to_treasury_ratio: 5_000, // 50%
        default_grace_period: 0,
        default_penalty_bps: 2_000, // 20%
        editing_period: EDITING,
        voting_period: VOTING_PERIOD,
        treasury_threshold: 5_100, // 51%
        quorum_bps: 0,
    }
}

pub(crate) fn setup(num_members: u32) -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_id = sac.address();
    let token = token::Client::new(&env, &token_id);
    let token_mint = token::StellarAssetClient::new(&env, &token_id);

    let admin = Address::generate(&env);
    let contract_id = env.register(OurDao, ());
    let client = OurDaoClient::new(&env, &contract_id);

    let mut admins = Vec::new(&env);
    admins.push_back(admin.clone());
    client.initialize(&admins, &5_100u32, &FEE, &token_id, &policy());

    let mut members = Vec::new(&env);
    for _ in 0..num_members {
        let m = Address::generate(&env);
        token_mint.mint(&m, &MINT);
        client.register_member(&m);
        members.push_back(m);
    }

    Setup {
        env,
        client,
        token,
        admin,
        members,
    }
}

pub(crate) fn advance(env: &Env, secs: u64) {
    env.ledger().with_mut(|li| li.timestamp += secs);
}

// ---------------------------------------------------------------------------


/// True if any event from the most recent invocation has `name` as its first topic.
pub(crate) fn emitted(env: &Env, name: &str) -> bool {
    env.events().all().events().iter().any(|e| {
        let ContractEventBody::V0(body) = &e.body;
        matches!(body.topics.first(), Some(ScVal::Symbol(sym)) if sym.0.to_utf8_string_lossy() == name)
    })
}

/// Join a new member (mints their fee first), growing the treasury by FEE.

pub(crate) fn refill_treasury(s: &Setup) {
    let m = Address::generate(&s.env);
    token::StellarAssetClient::new(&s.env, &s.token.address).mint(&m, &MINT);
    s.client.register_member(&m);
}


pub(crate) fn init_with_token(env: &Env, token: &Address) -> Result<(), Error> {
    let contract_id = env.register(OurDao, ());
    let client = OurDaoClient::new(env, &contract_id);
    let mut admins = Vec::new(env);
    admins.push_back(Address::generate(env));
    match client.try_initialize(&admins, &5_100u32, &FEE, token, &policy()) {
        Ok(_) => Ok(()),
        Err(Ok(e)) => Err(e),
        Err(Err(_)) => panic!("unexpected host error"),
    }
}


mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// Keeps `amount * BASIS_POINTS` (BASIS_POINTS == 10_000) well clear of
    /// i128 overflow while still exercising values many orders of magnitude
    /// larger than any real loan or treasury.
    const AMOUNT_BOUND: i128 = i128::MAX / 20_000;

    fn contract_with_treasury(treasury: i128) -> (Env, OurDaoClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let token_admin = Address::generate(&env);
        let sac = env.register_stellar_asset_contract_v2(token_admin);
        let token_id = sac.address();
        let token_mint = token::StellarAssetClient::new(&env, &token_id);

        let admin = Address::generate(&env);
        let contract_id = env.register(OurDao, ());
        let client = OurDaoClient::new(&env, &contract_id);
        let mut admins = Vec::new(&env);
        admins.push_back(admin);
        client.initialize(&admins, &5_100u32, &FEE, &token_id, &policy());

        if treasury > 0 {
            token_mint.mint(&contract_id, &treasury);
        }
        (env, client)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(40))]

        /// `calculate_loan_terms`'s rate is a linear curve clamped to
        /// `[min_interest_rate, max_interest_rate]` — no amount or treasury
        /// size should ever push it outside that band, and the quoted
        /// repayment should never be less than the amount requested.
        #[test]
        fn loan_terms_rate_stays_within_policy_bounds(
            amount in 0i128..=AMOUNT_BOUND,
            treasury in 0i128..=AMOUNT_BOUND,
        ) {
            let (_env, client) = contract_with_treasury(treasury);
            let terms = client.calculate_loan_terms(&amount);
            let p = policy();
            prop_assert!(terms.interest_rate >= p.min_interest_rate);
            prop_assert!(terms.interest_rate <= p.max_interest_rate);
            prop_assert!(terms.total_repayment >= amount);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(80))]

        /// One base vote plus up to `MAX_STAKE_BONUS` (5) bonus votes at
        /// `STAKE_WEIGHT_UNIT` (100) tokens per bonus vote — so weight is
        /// always in `[1, 6]` for any non-negative stake.
        #[test]
        fn voting_weight_stays_in_bounds(stake in 0i128..=i128::MAX) {
            let env = Env::default();
            let contract_id = env.register(OurDao, ());
            let who = Address::generate(&env);
            let weight = env.as_contract(&contract_id, || {
                crate::storage::set_stake(&env, &who, stake);
                crate::util::voting_weight(&env, &who)
            });
            prop_assert!((1..=6).contains(&weight));
        }

        /// More stake never costs voting weight.
        #[test]
        fn voting_weight_is_monotonic_in_stake(
            a in 0i128..=i128::MAX,
            delta in 0i128..=i128::MAX,
        ) {
            let b = a.saturating_add(delta);
            let env = Env::default();
            let contract_id = env.register(OurDao, ());
            let who = Address::generate(&env);
            let (wa, wb) = env.as_contract(&contract_id, || {
                crate::storage::set_stake(&env, &who, a);
                let wa = crate::util::voting_weight(&env, &who);
                crate::storage::set_stake(&env, &who, b);
                let wb = crate::util::voting_weight(&env, &who);
                (wa, wb)
            });
            prop_assert!(wb >= wa);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        /// The pull-based accumulator splits `interest` equally across
        /// `active` members: every member ends up with exactly
        /// `interest / active` claimable, the sum never exceeds `interest`,
        /// and whatever isn't evenly divisible (`interest % active`) is
        /// exactly what's left retained by the treasury.
        #[test]
        fn distributed_interest_never_exceeds_collected(
            active in 1u32..=8,
            interest in 0i128..=i128::MAX,
        ) {
            let (env, client) = contract_with_treasury(0);
            let contract_id = client.address.clone();

            let mut members = Vec::new(&env);
            for _ in 0..active {
                let m = Address::generate(&env);
                let sac = token::StellarAssetClient::new(&env, &client.get_token());
                sac.mint(&m, &FEE);
                client.register_member(&m);
                members.push_back(m);
            }

            env.as_contract(&contract_id, || {
                crate::loans::distribute_interest(&env, interest);
            });

            let per_member = interest / active as i128;
            let mut sum = 0i128;
            for m in members.iter() {
                let pending = client.get_pending_yield(&m);
                prop_assert_eq!(pending, per_member);
                sum += pending;
            }
            prop_assert!(sum <= interest);
            prop_assert_eq!(interest - sum, interest % active as i128);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        /// `calculate_exit_share` is a pro-rata slice of the treasury.
        /// However a member's `contribution` got to its current value
        /// (join at the fee, then possibly reduced by default slashing —
        /// see issue's rejoin-after-exit note), each member's share is
        /// bounded by their contribution's fraction of the total, so the
        /// sum across every member can never exceed the treasury itself.
        #[test]
        fn exit_shares_never_exceed_treasury(
            contributions in prop::collection::vec(0i128..=FEE, 1..=5),
            extra_treasury in 0i128..=AMOUNT_BOUND,
        ) {
            let (env, client) = contract_with_treasury(extra_treasury);
            let contract_id = client.address.clone();
            let token_id = client.get_token();

            let mut members = Vec::new(&env);
            for &c in contributions.iter() {
                let m = Address::generate(&env);
                let sac = token::StellarAssetClient::new(&env, &token_id);
                sac.mint(&m, &FEE);
                client.register_member(&m);
                env.as_contract(&contract_id, || {
                    let mut rec = crate::storage::get_member(&env, &m).unwrap();
                    rec.contribution = c;
                    crate::storage::set_member(&env, &rec);
                });
                members.push_back(m);
            }

            let treasury = client.get_treasury_balance();
            let sum: i128 = members.iter().map(|m| client.calculate_exit_share(&m)).sum();
            prop_assert!(sum <= treasury);
        }
    }
}