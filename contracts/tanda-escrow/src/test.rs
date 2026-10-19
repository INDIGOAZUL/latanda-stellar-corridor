#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger, MockAuth, MockAuthInvoke},
    token, vec, Address, Env, IntoVal,
};

const START: u64 = 1_000;
const ROUND: u64 = 100;
const GRACE: u64 = 50;
const AMT: i128 = 100;
const MINT: i128 = 1_000;

struct T {
    env: Env,
    creator: Address,
    m: [Address; 3],
    outsider: Address,
    token_id: Address,
    cid: Address,
    gid: u64,
}

impl T {
    fn c(&self) -> TandaEscrowClient<'_> {
        TandaEscrowClient::new(&self.env, &self.cid)
    }
    fn tok(&self) -> token::TokenClient<'_> {
        token::TokenClient::new(&self.env, &self.token_id)
    }
    fn bal(&self, a: &Address) -> i128 {
        self.tok().balance(a)
    }
    fn set_time(&self, ts: u64) {
        self.env.ledger().with_mut(|l| l.timestamp = ts);
    }
    fn due(&self, r: u32) -> u64 {
        START + (r as u64 + 1) * ROUND
    }
    fn all_contribute(&self, r: u32) {
        for m in self.m.iter() {
            self.c().contribute(&self.gid, m, &r);
        }
    }
}

/// 3 members, payout order m[2], m[0], m[1]; contribution 100; rounds of 100s
/// starting at t=1000; grace 50s. Each member and an outsider hold 1000 tokens.
fn setup() -> T {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = START);

    let creator = Address::generate(&env);
    let m = [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    let outsider = Address::generate(&env);

    let issuer = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(issuer);
    let token_id = sac.address();
    let sa = token::StellarAssetClient::new(&env, &token_id);
    for a in m.iter() {
        sa.mint(a, &MINT);
    }
    sa.mint(&outsider, &MINT);

    let cid = env.register(TandaEscrow, ());
    let client = TandaEscrowClient::new(&env, &cid);
    let gid = client.create_group(
        &creator,
        &token_id,
        &vec![&env, m[0].clone(), m[1].clone(), m[2].clone()],
        &AMT,
        &vec![&env, m[2].clone(), m[0].clone(), m[1].clone()],
        &START,
        &ROUND,
        &GRACE,
    );
    T { env, creator, m, outsider, token_id, cid, gid }
}

#[test]
fn happy_path_3_members_3_rounds() {
    let t = setup();
    let order = [t.m[2].clone(), t.m[0].clone(), t.m[1].clone()];
    let mut received = [0i128; 3];
    for r in 0..3u32 {
        t.all_contribute(r);
        assert_eq!(t.bal(&t.cid), 3 * AMT);
        t.set_time(t.due(r));
        // release needs no signature at all: clear every auth and call it.
        t.env.set_auths(&[]);
        let paid = t.c().release(&t.gid, &r);
        assert_eq!(paid, 3 * AMT);
        assert_eq!(t.bal(&t.cid), 0);
        assert_eq!(t.c().get_round(&t.gid, &r).released, true);
        t.env.mock_all_auths();
        // The pot went to the beneficiary recorded for round r.
        let idx = t.m.iter().position(|m| *m == order[r as usize]).unwrap();
        received[idx] += paid;
        assert_eq!(t.bal(&order[r as usize]), MINT - (r as i128 + 1) * AMT + received[idx]);
    }
    // Everyone paid 300 and received 300.
    for m in t.m.iter() {
        assert_eq!(t.bal(m), MINT);
    }
    assert_eq!(t.bal(&t.cid), 0);
}

#[test]
fn release_pays_recorded_beneficiary_only() {
    let t = setup();
    t.all_contribute(0);
    t.set_time(t.due(0));
    t.c().release(&t.gid, &0);
    // Round 0 beneficiary is m[2]: paid 100, received 300.
    assert_eq!(t.bal(&t.m[2]), MINT - AMT + 3 * AMT);
    assert_eq!(t.bal(&t.m[0]), MINT - AMT);
    assert_eq!(t.bal(&t.m[1]), MINT - AMT);
    assert_eq!(t.bal(&t.creator), 0);
    assert_eq!(t.bal(&t.outsider), MINT);
}

#[test]
fn events_emitted() {
    let t = setup();
    t.c().contribute(&t.gid, &t.m[0], &0);
    assert!(!t.env.events().all().events().is_empty());
}

#[test]
fn contribute_moves_exact_amount_and_requires_member_auth() {
    let t = setup();
    t.c().contribute(&t.gid, &t.m[0], &0);
    // The member's signature was required for this call (checked right after
    // the invocation, before any other call resets the recorded auths).
    let auths = t.env.auths();
    assert!(auths.iter().any(|(a, _)| *a == t.m[0]));
    assert_eq!(t.bal(&t.m[0]), MINT - AMT);
    assert_eq!(t.bal(&t.cid), AMT);
}

#[test]
fn contribute_without_member_signature_fails() {
    let t = setup();
    t.env.set_auths(&[]);
    assert!(t.c().try_contribute(&t.gid, &t.m[0], &0).is_err());
    assert_eq!(t.bal(&t.m[0]), MINT);
}

#[test]
fn double_contribution_fails() {
    let t = setup();
    t.c().contribute(&t.gid, &t.m[0], &0);
    assert_eq!(
        t.c().try_contribute(&t.gid, &t.m[0], &0),
        Err(Ok(Error::AlreadyContributed))
    );
    assert_eq!(t.bal(&t.m[0]), MINT - AMT);
}

#[test]
fn non_member_contribution_fails() {
    let t = setup();
    assert_eq!(
        t.c().try_contribute(&t.gid, &t.outsider, &0),
        Err(Ok(Error::NotMember))
    );
    assert_eq!(
        t.c().try_contribute(&t.gid, &t.creator, &0),
        Err(Ok(Error::NotMember))
    );
}

#[test]
fn invalid_round_fails() {
    let t = setup();
    assert_eq!(
        t.c().try_contribute(&t.gid, &t.m[0], &3),
        Err(Ok(Error::InvalidRound))
    );
    assert_eq!(t.c().try_release(&t.gid, &3), Err(Ok(Error::InvalidRound)));
}

#[test]
fn contribution_after_grace_fails() {
    let t = setup();
    t.set_time(t.due(0) + GRACE - 1); // late but within grace: accepted
    t.c().contribute(&t.gid, &t.m[0], &0);
    t.set_time(t.due(0) + GRACE);
    assert_eq!(
        t.c().try_contribute(&t.gid, &t.m[1], &0),
        Err(Ok(Error::ContributionWindowClosed))
    );
}

#[test]
fn release_before_due_fails() {
    let t = setup();
    t.all_contribute(0);
    t.set_time(t.due(0) - 1);
    assert_eq!(t.c().try_release(&t.gid, &0), Err(Ok(Error::RoundNotDue)));
    assert_eq!(t.bal(&t.cid), 3 * AMT);
}

#[test]
fn release_incomplete_round_fails() {
    let t = setup();
    t.c().contribute(&t.gid, &t.m[0], &0);
    t.c().contribute(&t.gid, &t.m[1], &0);
    t.set_time(t.due(0));
    assert_eq!(
        t.c().try_release(&t.gid, &0),
        Err(Ok(Error::RoundIncomplete))
    );
    assert_eq!(t.bal(&t.cid), 2 * AMT);
}

#[test]
fn double_release_fails() {
    let t = setup();
    t.all_contribute(0);
    t.set_time(t.due(0));
    t.c().release(&t.gid, &0);
    assert_eq!(
        t.c().try_release(&t.gid, &0),
        Err(Ok(Error::AlreadyReleased))
    );
    assert_eq!(t.bal(&t.m[2]), MINT + 2 * AMT);
}

#[test]
fn refund_after_grace_returns_only_own_contribution() {
    let t = setup();
    t.c().contribute(&t.gid, &t.m[0], &0);
    t.c().contribute(&t.gid, &t.m[1], &0);
    // m[2] never pays round 0.
    t.set_time(t.due(0) + GRACE);
    assert_eq!(t.c().refund(&t.gid, &t.m[0], &0), AMT);
    assert_eq!(t.bal(&t.m[0]), MINT);
    assert_eq!(t.bal(&t.m[1]), MINT - AMT); // untouched
    assert_eq!(t.bal(&t.cid), AMT); // m[1]'s money stays until m[1] asks
    // Not twice.
    assert_eq!(
        t.c().try_refund(&t.gid, &t.m[0], &0),
        Err(Ok(Error::NothingToRefund))
    );
    // A member who never paid gets nothing.
    assert_eq!(
        t.c().try_refund(&t.gid, &t.m[2], &0),
        Err(Ok(Error::NothingToRefund))
    );
    // The incomplete round can never be released.
    assert_eq!(
        t.c().try_release(&t.gid, &0),
        Err(Ok(Error::RoundIncomplete))
    );
    t.c().refund(&t.gid, &t.m[1], &0);
    assert_eq!(t.bal(&t.m[1]), MINT);
    assert_eq!(t.bal(&t.cid), 0);
}

#[test]
fn refund_before_grace_fails() {
    let t = setup();
    t.c().contribute(&t.gid, &t.m[0], &0);
    t.set_time(t.due(0) + GRACE - 1);
    assert_eq!(
        t.c().try_refund(&t.gid, &t.m[0], &0),
        Err(Ok(Error::RefundNotOpen))
    );
}

#[test]
fn refund_of_complete_round_fails() {
    let t = setup();
    t.all_contribute(0);
    t.set_time(t.due(0) + GRACE + 1_000);
    assert_eq!(
        t.c().try_refund(&t.gid, &t.m[0], &0),
        Err(Ok(Error::RoundComplete))
    );
    // It can still be released to its beneficiary, by anyone.
    t.c().release(&t.gid, &0);
    assert_eq!(t.bal(&t.m[2]), MINT + 2 * AMT);
}

#[test]
fn refund_of_another_member_fails() {
    let t = setup();
    t.c().contribute(&t.gid, &t.m[0], &0);
    t.c().contribute(&t.gid, &t.m[1], &0);
    t.set_time(t.due(0) + GRACE);

    // m[0] signs a call that tries to refund m[1]'s contribution: rejected,
    // because the contract requires m[1]'s own signature.
    let args = (t.gid, t.m[1].clone(), 0u32).into_val(&t.env);
    let r = t
        .c()
        .mock_auths(&[MockAuth {
            address: &t.m[0],
            invoke: &MockAuthInvoke {
                contract: &t.cid,
                fn_name: "refund",
                args,
                sub_invokes: &[],
            },
        }])
        .try_refund(&t.gid, &t.m[1], &0);
    assert!(r.is_err());
    assert_eq!(t.bal(&t.cid), 2 * AMT);
    assert_eq!(t.bal(&t.m[1]), MINT - AMT);
}

/// There is no admin. The group creator, an outsider, or anyone without the
/// member's signature cannot move contributed funds anywhere.
#[test]
fn creator_and_outsiders_cannot_move_funds() {
    let t = setup();
    t.c().contribute(&t.gid, &t.m[0], &0);
    t.c().contribute(&t.gid, &t.m[1], &0);
    t.set_time(t.due(0) + GRACE);

    // Nobody signs: refunds on behalf of members fail.
    t.env.set_auths(&[]);
    assert!(t.c().try_refund(&t.gid, &t.m[0], &0).is_err());
    assert!(t.c().try_refund(&t.gid, &t.m[1], &0).is_err());
    // Creator / outsider calling refund for themselves: they have nothing in it.
    t.env.mock_all_auths();
    assert_eq!(
        t.c().try_refund(&t.gid, &t.creator, &0),
        Err(Ok(Error::NothingToRefund))
    );
    assert_eq!(
        t.c().try_refund(&t.gid, &t.outsider, &0),
        Err(Ok(Error::NothingToRefund))
    );
    // Release of an incomplete round fails for anyone.
    assert_eq!(
        t.c().try_release(&t.gid, &0),
        Err(Ok(Error::RoundIncomplete))
    );
    // Funds untouched; creator and outsider got nothing.
    assert_eq!(t.bal(&t.cid), 2 * AMT);
    assert_eq!(t.bal(&t.creator), 0);
    assert_eq!(t.bal(&t.outsider), MINT);

    // A complete round released by an outsider still pays the recorded
    // beneficiary, never the caller.
    t.all_contribute(1);
    t.set_time(t.due(1));
    t.env.set_auths(&[]);
    t.c().release(&t.gid, &1);
    assert_eq!(t.bal(&t.outsider), MINT);
    assert_eq!(t.bal(&t.creator), 0);
    // Round 1 beneficiary is m[0].
    assert_eq!(t.bal(&t.m[0]), MINT - 2 * AMT + 3 * AMT);
}

#[test]
fn rules_are_immutable_after_create() {
    let t = setup();
    let before = t.c().get_group(&t.gid);
    // Run a full round and create a second group with different rules.
    t.all_contribute(0);
    t.set_time(t.due(0));
    t.c().release(&t.gid, &0);
    let gid2 = t.c().create_group(
        &t.outsider,
        &t.token_id,
        &vec![&t.env, t.m[0].clone(), t.outsider.clone()],
        &7,
        &vec![&t.env, t.outsider.clone(), t.m[0].clone()],
        &0,
        &10,
        &0,
    );
    assert_ne!(gid2, t.gid);
    let after = t.c().get_group(&t.gid);
    assert_eq!(before, after);
    assert_eq!(after.contribution, AMT);
    assert_eq!(
        after.payout_order,
        vec![&t.env, t.m[2].clone(), t.m[0].clone(), t.m[1].clone()]
    );
}

#[test]
fn create_group_requires_creator_signature() {
    let t = setup();
    t.env.set_auths(&[]);
    let r = t.c().try_create_group(
        &t.creator,
        &t.token_id,
        &vec![&t.env, t.m[0].clone(), t.m[1].clone()],
        &AMT,
        &vec![&t.env, t.m[0].clone(), t.m[1].clone()],
        &START,
        &ROUND,
        &GRACE,
    );
    assert!(r.is_err());
}

#[test]
fn create_group_rejects_bad_rules() {
    let t = setup();
    let e = &t.env;
    let (a, b, x) = (t.m[0].clone(), t.m[1].clone(), t.outsider.clone());
    let mk = |members: Vec<Address>, order: Vec<Address>, amt: i128, secs: u64| {
        t.c().try_create_group(&t.creator, &t.token_id, &members, &amt, &order, &START, &secs, &GRACE)
    };
    // duplicate member
    assert_eq!(mk(vec![e, a.clone(), a.clone()], vec![e, a.clone(), a.clone()], AMT, ROUND), Err(Ok(Error::InvalidRules)));
    // beneficiary not a member
    assert_eq!(mk(vec![e, a.clone(), b.clone()], vec![e, a.clone(), x.clone()], AMT, ROUND), Err(Ok(Error::InvalidRules)));
    // beneficiary repeated (not a permutation)
    assert_eq!(mk(vec![e, a.clone(), b.clone()], vec![e, a.clone(), a.clone()], AMT, ROUND), Err(Ok(Error::InvalidRules)));
    // wrong length
    assert_eq!(mk(vec![e, a.clone(), b.clone()], vec![e, a.clone()], AMT, ROUND), Err(Ok(Error::InvalidRules)));
    // single member
    assert_eq!(mk(vec![e, a.clone()], vec![e, a.clone()], AMT, ROUND), Err(Ok(Error::InvalidRules)));
    // zero / negative contribution, zero round length
    assert_eq!(mk(vec![e, a.clone(), b.clone()], vec![e, b.clone(), a.clone()], 0, ROUND), Err(Ok(Error::InvalidRules)));
    assert_eq!(mk(vec![e, a.clone(), b.clone()], vec![e, b.clone(), a.clone()], -5, ROUND), Err(Ok(Error::InvalidRules)));
    assert_eq!(mk(vec![e, a.clone(), b.clone()], vec![e, b.clone(), a.clone()], AMT, 0), Err(Ok(Error::InvalidRules)));
}

#[test]
fn unknown_group_fails() {
    let t = setup();
    assert_eq!(t.c().try_release(&99, &0), Err(Ok(Error::GroupNotFound)));
    assert_eq!(
        t.c().try_contribute(&99, &t.m[0], &0),
        Err(Ok(Error::GroupNotFound))
    );
}
