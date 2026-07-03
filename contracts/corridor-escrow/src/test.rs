#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

/// Owned test fixture. We deliberately store only owned values (Env is cheap to
/// clone) and build the typed clients inside each test, to avoid a
/// self-referential struct.
struct Setup {
    env: Env,
    admin: Address,
    sender: Address,
    recipient: Address,
    token_id: Address,
    contract_id: Address,
}

impl Setup {
    fn token(&self) -> token::TokenClient {
        token::TokenClient::new(&self.env, &self.token_id)
    }
    fn escrow(&self) -> CorridorEscrowClient {
        CorridorEscrowClient::new(&self.env, &self.contract_id)
    }
}

fn setup() -> Setup {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    // Issue a mock USDC via a Stellar Asset Contract and mint to the sender.
    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token_id = sac.address();
    token::StellarAssetClient::new(&env, &token_id).mint(&sender, &1_000);

    // Deploy the escrow and initialize it with the coordinator + token.
    let contract_id = env.register(CorridorEscrow, ());
    CorridorEscrowClient::new(&env, &contract_id).initialize(&admin, &token_id);

    Setup { env, admin, sender, recipient, token_id, contract_id }
}

#[test]
fn lock_then_release_pays_recipient() {
    let s = setup();
    let (escrow, token) = (s.escrow(), s.token());

    // Sender locks 400 USDC to the recipient, expiring at ledger 5_000.
    escrow.lock(&s.sender, &1, &s.recipient, &400, &5_000);

    // Funds left the sender and are held by the escrow contract.
    assert_eq!(token.balance(&s.sender), 600);
    assert_eq!(token.balance(&s.contract_id), 400);
    assert_eq!(token.balance(&s.recipient), 0);

    let e = escrow.get_escrow(&1);
    assert_eq!(e.status, Status::Funded);
    assert_eq!(e.amount, 400);

    // Coordinator releases: funds reach the pre-recorded recipient.
    escrow.release(&1);
    assert_eq!(token.balance(&s.recipient), 400);
    assert_eq!(token.balance(&s.contract_id), 0);
    assert_eq!(escrow.get_escrow(&1).status, Status::Released);
}

#[test]
fn lock_then_timeout_refunds_sender() {
    let s = setup();
    let (escrow, token) = (s.escrow(), s.token());

    escrow.lock(&s.sender, &7, &s.recipient, &250, &100);
    assert_eq!(token.balance(&s.sender), 750);

    // Advance the ledger past expiry, then the sender self-refunds.
    s.env.ledger().with_mut(|l| l.sequence_number = 101);
    escrow.refund(&s.sender, &7);

    assert_eq!(token.balance(&s.sender), 1_000);
    assert_eq!(token.balance(&s.contract_id), 0);
    assert_eq!(escrow.get_escrow(&7).status, Status::Refunded);
}

#[test]
fn refund_before_expiry_is_rejected() {
    let s = setup();
    let (escrow, token) = (s.escrow(), s.token());

    escrow.lock(&s.sender, &9, &s.recipient, &100, &10_000);

    // Current ledger is well before expiry (10_000).
    let res = escrow.try_refund(&s.sender, &9);
    assert_eq!(res, Err(Ok(Error::NotYetExpired)));

    // Still funded, nothing moved.
    assert_eq!(escrow.get_escrow(&9).status, Status::Funded);
    assert_eq!(token.balance(&s.contract_id), 100);
}

#[test]
fn refund_by_stranger_is_rejected() {
    let s = setup();
    let escrow = s.escrow();

    escrow.lock(&s.sender, &11, &s.recipient, &100, &1);
    s.env.ledger().with_mut(|l| l.sequence_number = 2);

    // A third party (neither sender nor admin) cannot refund, even after expiry.
    let stranger = Address::generate(&s.env);
    let res = escrow.try_refund(&stranger, &11);
    assert_eq!(res, Err(Ok(Error::InvalidStatus)));
    assert_eq!(escrow.get_escrow(&11).status, Status::Funded);
}

#[test]
fn double_release_is_rejected() {
    let s = setup();
    let escrow = s.escrow();

    escrow.lock(&s.sender, &13, &s.recipient, &100, &5_000);
    escrow.release(&13);

    // A second release on an already-released escrow fails.
    let res = escrow.try_release(&13);
    assert_eq!(res, Err(Ok(Error::InvalidStatus)));
}

#[test]
fn duplicate_escrow_id_is_rejected() {
    let s = setup();
    let escrow = s.escrow();

    escrow.lock(&s.sender, &21, &s.recipient, &100, &5_000);
    let res = escrow.try_lock(&s.sender, &21, &s.recipient, &50, &5_000);
    assert_eq!(res, Err(Ok(Error::EscrowAlreadyExists)));
}

#[test]
fn zero_amount_is_rejected() {
    let s = setup();
    let res = s.escrow().try_lock(&s.sender, &31, &s.recipient, &0, &5_000);
    assert_eq!(res, Err(Ok(Error::InvalidAmount)));
}

#[test]
#[should_panic] // admin.require_auth() is enforced once mocked auths are cleared
fn release_without_admin_auth_panics() {
    let s = setup();
    let escrow = s.escrow();
    escrow.lock(&s.sender, &41, &s.recipient, &100, &5_000);

    // Clear all mocked authorizations so require_auth() is enforced for real;
    // release() demands the admin's signature and none is provided.
    s.env.set_auths(&[]);
    escrow.release(&41);
}

#[test]
fn double_initialize_is_rejected() {
    let s = setup();
    let other = Address::generate(&s.env);
    let res = s.escrow().try_initialize(&other, &s.token_id);
    assert_eq!(res, Err(Ok(Error::AlreadyInitialized)));
}
