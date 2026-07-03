#![no_std]
//! # Corridor Escrow
//!
//! A **non-custodial** USDC escrow contract for La Tanda's cross-border
//! remittance / rotating-savings (tanda) payout corridor on Stellar (Soroban).
//!
//! ## What it does
//! A diaspora **sender** locks USDC into an escrow keyed by a `corridor/payout id`.
//! The funds can only ever move to two pre-committed destinations:
//!  * **release** -> to the `recipient` recorded at lock time (the tanda cycle
//!    beneficiary, or the anchor / MoneyGram payout address that cashes them out), or
//!  * **refund**  -> back to the original `sender`, once the escrow has expired.
//!
//! ## Non-custodial design
//! The contract holds funds *in escrow per corridor logic only*. La Tanda (as the
//! `admin` / coordinator) can **trigger a release to the pre-recorded recipient**
//! or a refund — it can **never** re-address funds to an arbitrary account or
//! sweep the balance. Destinations are fixed at `lock` time by the sender. This
//! keeps custody with the escrow rules, not with La Tanda.
//!
//! The regulated fiat cash-in / cash-out legs are performed off-chain by a
//! **licensed Stellar anchor / MoneyGram Ramps**, not by this contract.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env, Symbol,
};

/// Storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Contract configuration (admin + USDC token address). Instance storage.
    Config,
    /// A single escrow record, keyed by its corridor/payout id. Persistent storage.
    Escrow(u64),
}

/// Lifecycle status of an escrow.
#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// Funds are locked and awaiting release or refund.
    Funded = 0,
    /// Funds were released to the recipient.
    Released = 1,
    /// Funds were refunded to the sender after expiry.
    Refunded = 2,
}

/// Contract configuration set once at initialization.
#[contracttype]
#[derive(Clone)]
pub struct Config {
    /// Coordinator address allowed to trigger release/refund per the rules.
    pub admin: Address,
    /// The escrowed asset (e.g. USDC SAC contract address on this network).
    pub token: Address,
}

/// A single corridor escrow record.
#[contracttype]
#[derive(Clone)]
pub struct Escrow {
    /// Who funded the escrow (and who a refund returns to).
    pub sender: Address,
    /// Pre-committed payout destination (beneficiary or anchor payout address).
    pub recipient: Address,
    /// Amount of `token` held, in the token's smallest unit (stroops for USDC).
    pub amount: i128,
    /// Ledger sequence at/after which the escrow may be refunded to the sender.
    pub expiry_ledger: u32,
    /// Current lifecycle status.
    pub status: Status,
}

#[contracterror]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum Error {
    /// initialize() was called more than once.
    AlreadyInitialized = 1,
    /// A read expected the contract to be initialized, but it was not.
    NotInitialized = 2,
    /// No escrow exists for the supplied id.
    EscrowNotFound = 3,
    /// An escrow with this id already exists (ids are single-use).
    EscrowAlreadyExists = 4,
    /// Operation not valid for the escrow's current status (e.g. already released).
    InvalidStatus = 5,
    /// Amount must be strictly positive.
    InvalidAmount = 6,
    /// Refund attempted before the escrow's expiry ledger.
    NotYetExpired = 7,
}

/// Events emitted for each state transition, so off-chain services (and the
/// Cosmos-side settlement coordinator) can react to corridor lifecycle changes.
const INIT: Symbol = symbol_short!("init");
const LOCK: Symbol = symbol_short!("lock");
const RELEASE: Symbol = symbol_short!("release");
const REFUND: Symbol = symbol_short!("refund");

#[contract]
pub struct CorridorEscrow;

#[contractimpl]
impl CorridorEscrow {
    /// One-time setup. `admin` is the La Tanda coordinator that may trigger
    /// release/refund; `token` is the escrowed asset (USDC SAC address).
    pub fn initialize(env: Env, admin: Address, token: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage()
            .instance()
            .set(&DataKey::Config, &Config { admin: admin.clone(), token: token.clone() });
        env.events().publish((INIT, admin), token);
        Ok(())
    }

    /// **Lock** — the sender escrows `amount` of USDC under `escrow_id`, committing
    /// now to a single `recipient` and an `expiry_ledger` for auto-refund.
    ///
    /// Requires the sender's authorization; pulls the funds from the sender into
    /// this contract via the token contract's `transfer`.
    pub fn lock(
        env: Env,
        sender: Address,
        escrow_id: u64,
        recipient: Address,
        amount: i128,
        expiry_ledger: u32,
    ) -> Result<(), Error> {
        // Only the sender can move their own funds into escrow.
        sender.require_auth();

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let config = Self::load_config(&env)?;

        let key = DataKey::Escrow(escrow_id);
        if env.storage().persistent().has(&key) {
            return Err(Error::EscrowAlreadyExists);
        }

        // Pull USDC from the sender into the escrow (contract) balance.
        let token_client = token::TokenClient::new(&env, &config.token);
        token_client.transfer(&sender, &env.current_contract_address(), &amount);

        let escrow = Escrow {
            sender: sender.clone(),
            recipient: recipient.clone(),
            amount,
            expiry_ledger,
            status: Status::Funded,
        };
        env.storage().persistent().set(&key, &escrow);

        env.events()
            .publish((LOCK, escrow_id, sender), (recipient, amount, expiry_ledger));
        Ok(())
    }

    /// **Release** — the admin/coordinator releases a funded escrow to its
    /// pre-recorded `recipient`. The destination is fixed at lock time; the admin
    /// cannot redirect funds, only trigger the pre-committed payout.
    pub fn release(env: Env, escrow_id: u64) -> Result<(), Error> {
        let config = Self::load_config(&env)?;
        // Authorization: only the coordinator may trigger a release.
        config.admin.require_auth();

        let key = DataKey::Escrow(escrow_id);
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::EscrowNotFound)?;

        if escrow.status != Status::Funded {
            return Err(Error::InvalidStatus);
        }

        let token_client = token::TokenClient::new(&env, &config.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.recipient,
            &escrow.amount,
        );

        escrow.status = Status::Released;
        env.storage().persistent().set(&key, &escrow);

        env.events()
            .publish((RELEASE, escrow_id, escrow.recipient), escrow.amount);
        Ok(())
    }

    /// **Refund** — after `expiry_ledger`, return a still-funded escrow to its
    /// sender. Callable by either the sender (self-service) or the admin. Like
    /// release, the destination (the original sender) is fixed and cannot be
    /// redirected.
    pub fn refund(env: Env, caller: Address, escrow_id: u64) -> Result<(), Error> {
        caller.require_auth();
        let config = Self::load_config(&env)?;

        let key = DataKey::Escrow(escrow_id);
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::EscrowNotFound)?;

        // Only the sender or the coordinator may trigger a refund.
        if caller != escrow.sender && caller != config.admin {
            return Err(Error::InvalidStatus);
        }
        if escrow.status != Status::Funded {
            return Err(Error::InvalidStatus);
        }
        if env.ledger().sequence() < escrow.expiry_ledger {
            return Err(Error::NotYetExpired);
        }

        let token_client = token::TokenClient::new(&env, &config.token);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.sender,
            &escrow.amount,
        );

        escrow.status = Status::Refunded;
        env.storage().persistent().set(&key, &escrow);

        env.events()
            .publish((REFUND, escrow_id, escrow.sender), escrow.amount);
        Ok(())
    }

    /// Read a single escrow record.
    pub fn get_escrow(env: Env, escrow_id: u64) -> Result<Escrow, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Escrow(escrow_id))
            .ok_or(Error::EscrowNotFound)
    }

    /// Read the contract configuration (admin + token).
    pub fn get_config(env: Env) -> Result<Config, Error> {
        Self::load_config(&env)
    }

    fn load_config(env: &Env) -> Result<Config, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(Error::NotInitialized)
    }
}

#[cfg(test)]
mod test;
