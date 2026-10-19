#![no_std]
//! # Tanda Escrow (v2)
//!
//! A rotating-savings group ("tanda") escrow for Stellar / Soroban.
//!
//! ## What it does
//! * `create_group` records the group's rules on-chain: members, the
//!   contribution per round, the payout order (one beneficiary per round),
//!   the round schedule (start + round length, in ledger-timestamp seconds),
//!   the grace period and the token. **There is no function that changes
//!   these rules afterwards.**
//! * `contribute` moves exactly one contribution from a member's own wallet
//!   into the contract. It requires that member's signature, accepts only
//!   members, and only once per member and round.
//! * `release` can be called by **any account** (no signature needed). It pays
//!   the full pot of a round to the beneficiary recorded for that round, and
//!   only if the round is fully funded and its due time has passed. A round is
//!   paid at most once.
//! * `refund` lets a member who contributed to a round that did **not** get
//!   fully funded take back **their own** contribution, once the due time plus
//!   the grace period has passed. It requires that member's signature.
//!
//! ## Who can move money
//! There is **no admin role** in this contract. Nobody at La Tanda (and not the
//! group creator) can withdraw, redirect, pause or sweep funds. Money held by
//! the contract can only leave through two paths whose destination is fixed by
//! the rules:
//! * `release` -> the beneficiary recorded for that round at `create_group`, or
//! * `refund`  -> back to the member who contributed it.
//!
//! ## Limits (stated plainly)
//! * The contract has **not been audited**.
//! * It does not force anyone to contribute. If a member does not pay, the round
//!   stays incomplete and, after the grace period, the members who did pay can
//!   take their own money back. Collecting from a member who did not pay is an
//!   off-chain (social / legal) matter.
//! * Someone has to submit the `release` transaction (any account can, e.g. the
//!   beneficiary or La Tanda's app). Nothing executes on its own on Stellar; what
//!   the contract guarantees is that *who* submits it cannot change *where* the
//!   money goes or *when* it can go.
//! * Persistent storage on Soroban has a time-to-live. The contract extends it
//!   on every write, but a group left untouched for months may need a TTL
//!   restore before it can be used again.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, Env, Vec,
};

/// Storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Next group id (instance storage).
    NextGroupId,
    /// Immutable rules of a group (persistent).
    Group(u64),
    /// Per-round state of a group (persistent).
    Round(u64, u32),
    /// Contribution state of one member in one round (persistent).
    Contribution(u64, u32, Address),
}

/// Rules of a tanda group. Written once by `create_group`, never modified.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// Account that created (and signed for) the group. Has no special power
    /// after creation.
    pub creator: Address,
    /// Token used for contributions and payouts (e.g. the USDC SAC address).
    pub token: Address,
    /// Members of the group. Every member contributes every round.
    pub members: Vec<Address>,
    /// Exact contribution per member per round, in the token's smallest unit.
    pub contribution: i128,
    /// `payout_order[r]` is the beneficiary of round `r`. It is a permutation of
    /// `members`, so the number of rounds equals the number of members.
    pub payout_order: Vec<Address>,
    /// Ledger timestamp (unix seconds) at which round 0 starts.
    pub start_ts: u64,
    /// Length of each round in seconds. Round `r` is due at
    /// `start_ts + (r + 1) * round_secs`.
    pub round_secs: u64,
    /// Seconds after a round's due time during which late contributions are
    /// still accepted; after it, an incomplete round can only be refunded.
    pub grace_secs: u64,
}

/// State of one round.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Round {
    /// Number of members who have contributed (net of refunds).
    pub paid_count: u32,
    /// Whether the pot has been paid to the beneficiary.
    pub released: bool,
}

/// State of one member's contribution in one round.
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContributionState {
    Paid = 0,
    Refunded = 1,
}

#[contracterror]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum Error {
    /// The group does not exist.
    GroupNotFound = 1,
    /// Invalid rules passed to create_group (see docs on create_group).
    InvalidRules = 2,
    /// The address is not a member of the group.
    NotMember = 3,
    /// Round index is out of range for this group.
    InvalidRound = 4,
    /// This member already contributed to this round.
    AlreadyContributed = 5,
    /// Contributions for this round are closed (due time + grace passed).
    ContributionWindowClosed = 6,
    /// The round's due time has not been reached yet.
    RoundNotDue = 7,
    /// The round is not fully funded.
    RoundIncomplete = 8,
    /// The round was already paid out.
    AlreadyReleased = 9,
    /// Refunds are not open yet (due time + grace not reached).
    RefundNotOpen = 10,
    /// The round is fully funded, so it is paid out, not refunded.
    RoundComplete = 11,
    /// This member has no contribution to refund in this round.
    NothingToRefund = 12,
    /// Arithmetic overflow while computing amounts or times.
    Overflow = 13,
}

// ---------------------------------------------------------------- events ----

/// Emitted by `create_group`. Topics: `["group_created", group_id]`.
#[contractevent]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupCreated {
    #[topic]
    pub group_id: u64,
    pub creator: Address,
    pub token: Address,
    pub members: Vec<Address>,
    pub payout_order: Vec<Address>,
    pub contribution: i128,
    pub start_ts: u64,
    pub round_secs: u64,
    pub grace_secs: u64,
}

/// Emitted by `contribute`. Topics: `["contributed", group_id, round, member]`.
#[contractevent]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contributed {
    #[topic]
    pub group_id: u64,
    #[topic]
    pub round: u32,
    #[topic]
    pub member: Address,
    pub amount: i128,
    pub paid_count: u32,
}

/// Emitted by `release`. Topics: `["released", group_id, round, beneficiary]`.
#[contractevent]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Released {
    #[topic]
    pub group_id: u64,
    #[topic]
    pub round: u32,
    #[topic]
    pub beneficiary: Address,
    pub amount: i128,
}

/// Emitted by `refund`. Topics: `["refunded", group_id, round, member]`.
#[contractevent]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refunded {
    #[topic]
    pub group_id: u64,
    #[topic]
    pub round: u32,
    #[topic]
    pub member: Address,
    pub amount: i128,
}

// ------------------------------------------------------------- constants ----

/// TTL management for persistent entries (in ledgers, ~5s each).
/// Extend when fewer than ~30 days remain, up to ~180 days.
const TTL_THRESHOLD: u32 = 518_400;
const TTL_EXTEND_TO: u32 = 3_110_400;

#[contract]
pub struct TandaEscrow;

#[contractimpl]
impl TandaEscrow {
    /// Create a group with immutable rules. Signed by `creator`.
    ///
    /// Rules checked:
    /// * at least 2 members, no duplicates;
    /// * `payout_order` is a permutation of `members` (each member is the
    ///   beneficiary of exactly one round);
    /// * `contribution > 0`, `round_secs > 0`;
    /// * the pot (`contribution * members`) and all due times fit without overflow.
    ///
    /// Returns the new group id.
    #[allow(clippy::too_many_arguments)]
    pub fn create_group(
        env: Env,
        creator: Address,
        token: Address,
        members: Vec<Address>,
        contribution: i128,
        payout_order: Vec<Address>,
        start_ts: u64,
        round_secs: u64,
        grace_secs: u64,
    ) -> Result<u64, Error> {
        creator.require_auth();

        let n = members.len();
        if n < 2 || payout_order.len() != n || contribution <= 0 || round_secs == 0 {
            return Err(Error::InvalidRules);
        }
        // No duplicate members.
        for i in 0..n {
            let a = members.get_unchecked(i);
            for j in (i + 1)..n {
                if a == members.get_unchecked(j) {
                    return Err(Error::InvalidRules);
                }
            }
        }
        // payout_order is a permutation of members: same length, every entry is
        // a member, and no entry is repeated.
        for i in 0..n {
            let b = payout_order.get_unchecked(i);
            if !members.contains(&b) {
                return Err(Error::InvalidRules);
            }
            for j in (i + 1)..n {
                if b == payout_order.get_unchecked(j) {
                    return Err(Error::InvalidRules);
                }
            }
        }
        // Pot and the last refund-opening time must not overflow.
        contribution
            .checked_mul(n as i128)
            .ok_or(Error::Overflow)?;
        round_secs
            .checked_mul(n as u64)
            .and_then(|d| d.checked_add(start_ts))
            .and_then(|d| d.checked_add(grace_secs))
            .ok_or(Error::Overflow)?;

        let id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextGroupId)
            .unwrap_or(0);
        env.storage().instance().set(&DataKey::NextGroupId, &(id + 1));
        env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);

        let group = Group {
            creator: creator.clone(),
            token: token.clone(),
            members: members.clone(),
            contribution,
            payout_order: payout_order.clone(),
            start_ts,
            round_secs,
            grace_secs,
        };
        let key = DataKey::Group(id);
        env.storage().persistent().set(&key, &group);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);

        GroupCreated {
            group_id: id,
            creator,
            token,
            members,
            payout_order,
            contribution,
            start_ts,
            round_secs,
            grace_secs,
        }
        .publish(&env);
        Ok(id)
    }

    /// A member pays exactly `contribution` for `round`, from their own wallet,
    /// signed by that member. One contribution per member per round. Accepted
    /// any time before the round's due time + grace.
    pub fn contribute(env: Env, group_id: u64, member: Address, round: u32) -> Result<(), Error> {
        member.require_auth();
        let group = Self::load_group(&env, group_id)?;
        Self::check_round(&group, round)?;
        if !group.members.contains(&member) {
            return Err(Error::NotMember);
        }
        let now = env.ledger().timestamp();
        if now >= Self::refund_open_ts(&group, round)? {
            return Err(Error::ContributionWindowClosed);
        }
        let ckey = DataKey::Contribution(group_id, round, member.clone());
        if env.storage().persistent().has(&ckey) {
            return Err(Error::AlreadyContributed);
        }
        let rkey = DataKey::Round(group_id, round);
        let mut st = Self::load_round(&env, &rkey);
        if st.released {
            return Err(Error::AlreadyReleased);
        }

        token::TokenClient::new(&env, &group.token).transfer(
            &member,
            &env.current_contract_address(),
            &group.contribution,
        );

        st.paid_count += 1;
        Self::save(&env, &rkey, &st);
        Self::save(&env, &ckey, &ContributionState::Paid);

        Contributed {
            group_id,
            round,
            member,
            amount: group.contribution,
            paid_count: st.paid_count,
        }
        .publish(&env);
        Ok(())
    }

    /// Pay the full pot of `round` to the beneficiary recorded for it.
    ///
    /// **Callable by any account; requires no signature.** Succeeds only if the
    /// round's due time has passed, every member contributed, and the round has
    /// not been paid yet. The destination is not a parameter: it is
    /// `payout_order[round]`, fixed at `create_group`.
    pub fn release(env: Env, group_id: u64, round: u32) -> Result<i128, Error> {
        let group = Self::load_group(&env, group_id)?;
        Self::check_round(&group, round)?;
        let now = env.ledger().timestamp();
        if now < Self::due_ts(&group, round)? {
            return Err(Error::RoundNotDue);
        }
        let rkey = DataKey::Round(group_id, round);
        let mut st = Self::load_round(&env, &rkey);
        if st.released {
            return Err(Error::AlreadyReleased);
        }
        if st.paid_count != group.members.len() {
            return Err(Error::RoundIncomplete);
        }

        let beneficiary = group.payout_order.get_unchecked(round);
        let pot = group
            .contribution
            .checked_mul(group.members.len() as i128)
            .ok_or(Error::Overflow)?;

        // Effects before interaction.
        st.released = true;
        Self::save(&env, &rkey, &st);

        token::TokenClient::new(&env, &group.token).transfer(
            &env.current_contract_address(),
            &beneficiary,
            &pot,
        );

        Released {
            group_id,
            round,
            beneficiary,
            amount: pot,
        }
        .publish(&env);
        Ok(pot)
    }

    /// A member takes back **their own** contribution to a round that was not
    /// fully funded, once the round's due time + grace has passed. Signed by
    /// that member; the money can only go back to that member.
    pub fn refund(env: Env, group_id: u64, member: Address, round: u32) -> Result<i128, Error> {
        member.require_auth();
        let group = Self::load_group(&env, group_id)?;
        Self::check_round(&group, round)?;
        let now = env.ledger().timestamp();
        if now < Self::refund_open_ts(&group, round)? {
            return Err(Error::RefundNotOpen);
        }
        let rkey = DataKey::Round(group_id, round);
        let mut st = Self::load_round(&env, &rkey);
        if st.released {
            return Err(Error::AlreadyReleased);
        }
        // A fully funded round is never refunded: it can only be released.
        // (Contributions are closed at this point, so this cannot change.)
        if st.paid_count == group.members.len() {
            return Err(Error::RoundComplete);
        }
        let ckey = DataKey::Contribution(group_id, round, member.clone());
        let cs: Option<ContributionState> = env.storage().persistent().get(&ckey);
        if cs != Some(ContributionState::Paid) {
            return Err(Error::NothingToRefund);
        }

        st.paid_count -= 1;
        Self::save(&env, &rkey, &st);
        Self::save(&env, &ckey, &ContributionState::Refunded);

        token::TokenClient::new(&env, &group.token).transfer(
            &env.current_contract_address(),
            &member,
            &group.contribution,
        );

        Refunded {
            group_id,
            round,
            member,
            amount: group.contribution,
        }
        .publish(&env);
        Ok(group.contribution)
    }

    // ----------------------------------------------------------- views ----

    /// Read a group's (immutable) rules.
    pub fn get_group(env: Env, group_id: u64) -> Result<Group, Error> {
        Self::load_group(&env, group_id)
    }

    /// Read a round's state.
    pub fn get_round(env: Env, group_id: u64, round: u32) -> Result<Round, Error> {
        let group = Self::load_group(&env, group_id)?;
        Self::check_round(&group, round)?;
        Ok(Self::load_round(&env, &DataKey::Round(group_id, round)))
    }

    /// Read one member's contribution state for a round (None = not paid).
    pub fn get_contribution(
        env: Env,
        group_id: u64,
        round: u32,
        member: Address,
    ) -> Option<ContributionState> {
        env.storage()
            .persistent()
            .get(&DataKey::Contribution(group_id, round, member))
    }

    /// Due time (unix seconds) of a round: `start_ts + (round + 1) * round_secs`.
    pub fn round_due_ts(env: Env, group_id: u64, round: u32) -> Result<u64, Error> {
        let group = Self::load_group(&env, group_id)?;
        Self::check_round(&group, round)?;
        Self::due_ts(&group, round)
    }

    // --------------------------------------------------------- helpers ----

    fn load_group(env: &Env, group_id: u64) -> Result<Group, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Group(group_id))
            .ok_or(Error::GroupNotFound)
    }

    fn load_round(env: &Env, key: &DataKey) -> Round {
        env.storage().persistent().get(key).unwrap_or(Round {
            paid_count: 0,
            released: false,
        })
    }

    fn save<V: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(env: &Env, key: &DataKey, v: &V) {
        env.storage().persistent().set(key, v);
        env.storage()
            .persistent()
            .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }

    fn check_round(group: &Group, round: u32) -> Result<(), Error> {
        if round >= group.payout_order.len() {
            return Err(Error::InvalidRound);
        }
        Ok(())
    }

    fn due_ts(group: &Group, round: u32) -> Result<u64, Error> {
        (round as u64 + 1)
            .checked_mul(group.round_secs)
            .and_then(|d| d.checked_add(group.start_ts))
            .ok_or(Error::Overflow)
    }

    fn refund_open_ts(group: &Group, round: u32) -> Result<u64, Error> {
        Self::due_ts(group, round)?
            .checked_add(group.grace_secs)
            .ok_or(Error::Overflow)
    }
}

#[cfg(test)]
mod test;
