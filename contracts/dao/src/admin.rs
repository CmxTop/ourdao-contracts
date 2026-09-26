use soroban_sdk::{symbol_short, Address, Env, Vec};

use crate::error::Error;
use crate::storage::{self, extend_instance};
use crate::types::{LoanPolicy, BASIS_POINTS};
use crate::util;

fn validate_policy(policy: &LoanPolicy) -> Result<(), Error> {
    if policy.membership_contribution <= 0
        || policy.max_loan_duration == 0
        || policy.min_interest_rate as i128 > BASIS_POINTS
        || policy.max_interest_rate as i128 > BASIS_POINTS
        || policy.min_interest_rate > policy.max_interest_rate
        || policy.max_loan_to_treasury_ratio == 0
        || policy.max_loan_to_treasury_ratio as i128 > BASIS_POINTS
        || policy.default_penalty_bps as i128 > BASIS_POINTS
        || policy.editing_period == 0
        || policy.voting_period == 0
        || policy.editing_period > 30 * 24 * 60 * 60
        || policy.voting_period > 30 * 24 * 60 * 60
        || policy.min_membership_duration > 30 * 24 * 60 * 60
        || policy.cooldown_period > 30 * 24 * 60 * 60
        || policy.default_grace_period > 30 * 24 * 60 * 60
        || policy.treasury_threshold == 0
        || policy.treasury_threshold as i128 > BASIS_POINTS
    {
        return Err(Error::InvalidLoanPolicy);
    }
    Ok(())
}

pub fn initialize(
    env: &Env,
    admins: Vec<Address>,
    consensus_threshold: u32,
    membership_fee: i128,
    token: Address,
    policy: LoanPolicy,
) -> Result<(), Error> {
    if storage::is_initialized(env) {
        return Err(Error::AlreadyInitialized);
    }
    if consensus_threshold == 0 || consensus_threshold as i128 > BASIS_POINTS {
        return Err(Error::InvalidThreshold);
    }
    if membership_fee <= 0 {
        return Err(Error::InvalidAmount);
    }
    if admins.is_empty() {
        return Err(Error::NotAuthorized);
    }
    // Duplicates would let `remove_admin` (which drops every matching entry)
    // leave the contract with zero admins despite its last-admin guard (#42).
    for (i, a) in admins.iter().enumerate() {
        if admins.iter().skip(i + 1).any(|b| b == a) {
            return Err(Error::AlreadyAdmin);
        }
    }
    validate_policy(&policy)?;

    storage::set_admins(env, &admins);
    storage::set_threshold(env, consensus_threshold);
    storage::set_membership_fee(env, membership_fee);
    storage::set_token(env, &token);
    storage::set_policy(env, &policy);
    storage::set_paused(env, false);
    storage::set_members(env, &Vec::new(env));
    storage::set_total_members(env, 0);
    storage::set_active_members(env, 0);
    extend_instance(env);

    env.events().publish(
        (symbol_short!("init"),),
        (admins, consensus_threshold, membership_fee, token),
    );
    Ok(())
}

pub fn add_admin(env: &Env, caller: Address, admin: Address) -> Result<(), Error> {
    util::require_admin(env, &caller)?;
    let mut admins = storage::get_admins(env);
    if admins.iter().any(|a| a == admin) {
        return Err(Error::AlreadyAdmin);
    }
    admins.push_back(admin.clone());
    storage::set_admins(env, &admins);
    extend_instance(env);
    env.events().publish((symbol_short!("admin_add"),), admin);
    Ok(())
}

pub fn remove_admin(env: &Env, caller: Address, admin: Address) -> Result<(), Error> {
    util::require_admin(env, &caller)?;
    let admins = storage::get_admins(env);
    if admins.len() <= 1 {
        return Err(Error::CannotRemoveLastAdmin);
    }
    let mut next = Vec::new(env);
    let mut found = false;
    for a in admins.iter() {
        if a == admin {
            found = true;
        } else {
            next.push_back(a);
        }
    }
    if !found {
        return Err(Error::NotAdmin);
    }
    storage::set_admins(env, &next);
    extend_instance(env);
    env.events().publish((symbol_short!("admin_rem"),), admin);
    Ok(())
}

pub fn set_consensus_threshold(env: &Env, caller: Address, threshold: u32) -> Result<(), Error> {
    util::require_admin(env, &caller)?;
    if threshold == 0 || threshold as i128 > BASIS_POINTS {
        return Err(Error::InvalidThreshold);
    }
    storage::set_threshold(env, threshold);
    extend_instance(env);
    env.events()
        .publish((symbol_short!("threshold"),), threshold);
    Ok(())
}

pub fn set_policy(env: &Env, caller: Address, policy: LoanPolicy) -> Result<(), Error> {
    util::require_admin(env, &caller)?;
    validate_policy(&policy)?;
    storage::set_policy(env, &policy);
    extend_instance(env);
    env.events().publish((symbol_short!("policy"),), ());
    Ok(())
}

pub fn pause(env: &Env, caller: Address) -> Result<(), Error> {
    util::require_admin(env, &caller)?;
    if storage::is_paused(env) {
        return Err(Error::Paused);
    }
    storage::set_paused(env, true);
    extend_instance(env);
    env.events().publish((symbol_short!("paused"),), ());
    Ok(())
}

pub fn unpause(env: &Env, caller: Address) -> Result<(), Error> {
    util::require_admin(env, &caller)?;
    if !storage::is_paused(env) {
        return Err(Error::NotPaused);
    }
    storage::set_paused(env, false);
    extend_instance(env);
    env.events().publish((symbol_short!("unpaused"),), ());
    Ok(())
}
