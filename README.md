# La Tanda — Stellar Corridor Escrow

A **USDC corridor escrow smart contract** (Soroban / Rust, **Stellar testnet**) for
La Tanda's US → Honduras family-savings corridor: a diaspora sender in the US
locks USDC that can only be released to a pre-recorded recipient (or refunded to
the sender after expiry). **Destination-restricted escrow, operator-triggered
release. No mainnet deployment.**

The live La Tanda product (rotating-savings groups in Honduras) runs today on a
**Cosmos** app-chain and a Node.js/PostgreSQL backend; this repo is a **separate,
experimental Stellar rail** that is not wired into that product.

- Live product: https://latanda.online
- Live product figures (canonical): https://latanda.online/api/public/metrics
- Founder GitHub: https://github.com/INDIGOAZUL

---

## Status (Sep 2026)

- **Escrow contract:** built and unit-tested against `soroban-sdk 26.1.0`;
  deployed and exercised on **Stellar testnet** only. Not on mainnet, not in
  production.
- **Corridor to Honduras:** in design. The recipient-side cash-out for Honduras
  is an open dependency.
- **Licensed on/off-ramp:** in negotiation with a licensed partner. **No LOI or
  signed agreement yet.** The regulated fiat legs (KYC/AML, cash-in, cash-out)
  would sit with that licensed partner, not with La Tanda and not with this
  contract.
- **Stellar Community Fund #45:** La Tanda applied (Integration Track) and was
  **not selected**. The panel's feedback has been incorporated into the current
  design and into how this repo describes itself. The original application
  material is preserved unchanged under
  [`docs/historical-scf45/`](docs/historical-scf45/) for transparency; it does
  **not** describe current plans.
- **Live product numbers (2026-09-09, from the endpoint above):** 80 registered
  users (not KYC-verified), 5 groups, 41 completed real-money payouts,
  L 1,652,100 HNL (≈ US$63,500) cycled. Those numbers describe the Cosmos-side
  product, not anything in this repo.

---

## What this contract is

A **sender** locks USDC into an escrow keyed by a corridor/payout id. The funds
can only ever move to two destinations that are **fixed at lock time**:

- **release** → the pre-recorded `recipient` (the savings-group beneficiary, or
  a licensed ramp's payout address that cashes the funds out to fiat), or
- **refund** → back to the original `sender`, once the escrow expiry ledger passes.

An `admin` (the La Tanda **operator / coordinator**) can *trigger* a release to
the recipient or a refund per the rules, but can **never** re-address funds to
an arbitrary account or sweep the balance. The escrow rules, not the operator,
decide where the money can go.

### Custody note (plain language)

- The **contract** holds the USDC between `lock` and `release`/`refund`.
- The **operator** can only trigger the pre-committed outcomes; it cannot redirect
  or withdraw the funds.
- Fiat cash-in / cash-out would be performed off-chain by a **licensed ramp
  partner** (none contracted yet), not by La Tanda and not by this contract.

We describe this as *destination-restricted escrow with operator-triggered
release* and deliberately avoid stronger custody claims for it.

---

## How it would fit the corridor

```
                        US  →  Honduras   (corridor in design)

  ┌───────────┐   fiat in    ┌──────────────┐   USDC     ┌────────────────────────┐
  │  US-based │  (licensed   │  Stellar     │  lock()    │  Soroban               │
  │  sender   │ ───────────► │  wallet      │ ─────────► │  Corridor-Escrow       │
  │ (diaspora)│  ramp, TBD)  │              │            │  contract (this repo)  │
  └───────────┘              └──────────────┘            │                        │
                                                         │  status: FUNDED        │
                                                         └───────────┬────────────┘
                                                                     │
                                          operator triggers          │ release()
                                          the pre-committed payout    ▼
  ┌───────────┐   cash out    ┌──────────────┐   USDC     ┌────────────────────────┐
  │ recipient │  (licensed    │  ramp payout │ ◄───────── │  pays pre-recorded     │
  │ / family  │ ◄─────────────│  address     │            │  recipient (or ramp    │
  │  in HN    │  ramp, TBD)   │              │            │  payout address)       │
  └───────────┘               └──────────────┘            └────────────────────────┘

  On timeout (expiry_ledger passed) instead of release():  refund() → back to sender.

  Settlement boundary: the La Tanda rotating-savings ledger lives on the Cosmos
  app-chain (latanda-testnet-1). A Cosmos↔Stellar settlement coordinator that ties
  a group contribution/payout to this escrow leg is designed, not built.
```

---

## Contract API

| Function | Auth | Effect |
|---|---|---|
| `initialize(admin, token)` | — (once) | Sets the operator + escrowed asset (USDC SAC address). |
| `lock(sender, escrow_id, recipient, amount, expiry_ledger)` | `sender` | Pulls `amount` USDC from sender into escrow; records `recipient` + expiry; status → `Funded`. |
| `release(escrow_id)` | `admin` | Pays the **pre-recorded** recipient; status → `Released`. |
| `refund(caller, escrow_id)` | `sender` or `admin` | After `expiry_ledger`, returns funds to the **original sender**; status → `Refunded`. |
| `get_escrow(escrow_id)` / `get_config()` | — (view) | Read escrow / config state. |

Every state transition emits an event (`init` / `lock` / `release` / `refund`) so
off-chain services can react.

---

## Build, test, deploy (testnet)

Toolchain: **Rust** + the `wasm32-unknown-unknown` target + the **Stellar CLI**
(`stellar` / `soroban`). Contract targets **`soroban-sdk = 26.1.0`**.

```bash
# 1. Toolchain (once)
rustup target add wasm32-unknown-unknown
cargo install --locked stellar-cli    # provides the `stellar` (a.k.a. soroban) CLI

# 2. Unit tests (native)
cargo test                            # run from repo root or contracts/corridor-escrow

# 3. Build the optimized Wasm
stellar contract build
# artifact: target/wasm32-unknown-unknown/release/corridor_escrow.wasm

# 4. Deploy to Stellar TESTNET
stellar keys generate --global deployer --network testnet --fund
CID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/corridor_escrow.wasm \
  --source deployer --network testnet)
echo "Deployed: $CID"
```

A step-by-step `lock → release` / `lock → timeout → refund` walkthrough with real
CLI commands is preserved in
[`docs/historical-scf45/DEMO.md`](docs/historical-scf45/DEMO.md) (written for the
SCF application; the commands still work, the corridor framing there is
historical).

---

## Repository layout

```
contracts/corridor-escrow/
  Cargo.toml            # soroban-sdk 26.1.0, cdylib + rlib, release profile
  src/lib.rs            # the Corridor-Escrow contract
  src/test.rs           # unit tests (lock→release, timeout→refund, authz, edge cases)
Cargo.toml              # workspace
README.md               # this file
docs/historical-scf45/  # SCF #45 application material, preserved as submitted (not current plans)
  DEMO.md
  architecture.md
  STELLAR-INTEGRATION.md
  La-Tanda-SCF45-Deck-v2.pdf
LICENSE                 # Apache-2.0
```

---

## License

Apache-2.0 — see [`LICENSE`](LICENSE). Published as a reusable open-source escrow
pattern for rotating-savings / recurring cross-border payments.
