# La Tanda — Stellar Corridor Escrow

A **non-custodial USDC escrow smart contract** (Soroban / Rust) for La Tanda's
cross-border remittance and rotating-savings (**tanda**) payout corridor on
**Stellar**.

This repository is the open-source Soroban deliverable for La Tanda's **Stellar
Community Fund (SCF) Build Award** application (SCF #45, Integration Track). It is
the **v1 build scaffold funded by the grant — not a production or mainnet-deployed
system.** The live La Tanda product (rotating-savings groups, ~80 registered
users, ~US$30K cycled) runs today on a **Cosmos** app-chain; this repo is the
**net-new Stellar remittance rail** the grant funds us to build.

- Live product: https://latanda.online
- Founder GitHub: https://github.com/INDIGOAZUL

---

## What this contract is

A diaspora **sender** in the US locks USDC into an escrow keyed by a
corridor/payout id. The funds can only ever move to two destinations that are
**fixed at lock time**:

- **release** → the pre-recorded `recipient` (the tanda cycle beneficiary, or the
  anchor / MoneyGram payout address that cashes the funds out to fiat), or
- **refund** → back to the original `sender`, once the escrow expiry ledger passes.

An `admin` (the La Tanda **coordinator**) can *trigger* a release-to-recipient or
a refund per the rules — but can **never** re-address funds to an arbitrary
account or sweep the balance. This is what keeps the corridor **non-custodial in
spirit**: custody sits with the escrow rules, not with La Tanda.

### Non-custodial design note

La Tanda **never custodies user funds**. Members hold their own Stellar keys; the
USDC moves peer → escrow contract → peer (beneficiary or anchor payout address).
The regulated fiat cash-in / cash-out legs are performed off-chain by a
**licensed Stellar anchor / MoneyGram Ramps**, not by La Tanda and not by this
contract. This is both a deliberate design choice and a regulatory-risk mitigant.

---

## How it fits La Tanda's Stellar remittance corridor

```
                        US  →  El Salvador   (funded pilot corridor)

  ┌───────────┐   fiat in    ┌──────────────┐   USDC     ┌────────────────────────┐
  │  US-based │  (SEP-24 /   │  Stellar     │  lock()    │  Soroban               │
  │  sender   │ ───────────► │  anchor /    │ ─────────► │  Corridor-Escrow       │
  │ (diaspora)│  MoneyGram   │  wallet      │            │  contract (this repo)  │
  └───────────┘  cash-in     └──────────────┘            │                        │
                                                         │  status: FUNDED        │
                                                         └───────────┬────────────┘
                                                                     │
                                          coordinator triggers       │ release()
                                          the pre-committed payout    ▼
  ┌───────────┐   cash out    ┌──────────────┐   USDC     ┌────────────────────────┐
  │ recipient │  (MoneyGram   │  anchor /    │ ◄───────── │  pays pre-recorded     │
  │ / family  │ ◄─────────────│  MoneyGram   │            │  recipient (or anchor  │
  │ (unbanked)│  SEP-24/31    │  payout addr │            │  payout address)       │
  └───────────┘  in SV        └──────────────┘            └────────────────────────┘

  On timeout (expiry_ledger passed) instead of release():  refund() → back to sender.

  Settlement boundary: the La Tanda rotating-savings ledger lives on the Cosmos
  app-chain (latanda-testnet-1). A Cosmos↔Stellar settlement coordinator (grant
  Tranche #2) ties a tanda contribution/payout to this Stellar-side escrow leg.
```

---

## SCF milestone mapping

| Tranche | This repo's role |
|---|---|
| **#1 — MVP (testnet), US→El Salvador** | **This contract.** Corridor-Escrow v1 deployed to **Stellar testnet** with unit tests; CLI/demo of `lock → release` (and `lock → timeout → refund`) for the US→SV corridor. |
| **#2 — Testnet expansion, US→El Salvador** | Wrap this contract with **SEP-24** anchor on/off-ramp + **MoneyGram Ramps** cash-in/out (El Salvador — rails confirmed) + **SEP-31** cross-border, and the **Cosmos↔Stellar settlement** reference. Full "sender → escrow → beneficiary → MoneyGram cash-out" demo. |
| **#3 — Mainnet + Honduras expansion** | Deploy to **Stellar mainnet**; first real US→SV mainnet corridor transactions; secure a **Honduras** cash-out/anchor partner (open dependency) and extend the corridor to La Tanda's existing HN users. |

The full proposal summary lives in [`docs/architecture.md`](docs/architecture.md).

---

## Contract API

| Function | Auth | Effect |
|---|---|---|
| `initialize(admin, token)` | — (once) | Sets the coordinator + escrowed asset (USDC SAC address). |
| `lock(sender, escrow_id, recipient, amount, expiry_ledger)` | `sender` | Pulls `amount` USDC from sender into escrow; records `recipient` + expiry; status → `Funded`. |
| `release(escrow_id)` | `admin` | Pays the **pre-recorded** recipient; status → `Released`. |
| `refund(caller, escrow_id)` | `sender` or `admin` | After `expiry_ledger`, returns funds to the **original sender**; status → `Refunded`. |
| `get_escrow(escrow_id)` / `get_config()` | — (view) | Read escrow / config state. |

Every state transition emits an event (`init` / `lock` / `release` / `refund`) so
off-chain services and the Cosmos-side settlement coordinator can react.

---

## Build, test, deploy

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

# 4. Deploy to Stellar TESTNET (see DEMO.md for the full lock→release walkthrough)
stellar keys generate --global deployer --network testnet --fund
CID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/corridor_escrow.wasm \
  --source deployer --network testnet)
echo "Deployed: $CID"
```

See **[`DEMO.md`](DEMO.md)** for the end-to-end `lock → release` and
`lock → timeout → refund` demo with real CLI commands, and the founder recording
outline.

---

## Repository layout

```
contracts/corridor-escrow/
  Cargo.toml            # soroban-sdk 26.1.0, cdylib + rlib, release profile
  src/lib.rs            # the Corridor-Escrow contract
  src/test.rs           # unit tests (lock→release, timeout→refund, authz, edge cases)
Cargo.toml              # workspace
README.md               # this file
DEMO.md                 # step-by-step demo + founder recording outline
docs/architecture.md    # corridor architecture, SEP-24/31, MoneyGram, SV/HN sequencing
LICENSE                 # Apache-2.0
```

---

## Status & honesty

- This is the **grant build scaffold (v1)**, correct-by-inspection against
  `soroban-sdk 26.1.0`. It is **not** deployed to mainnet and **not** in
  production.
- No invented traction: the live-product numbers describe the **Cosmos** product
  that de-risks this build; the **Stellar** layer in this repo is the planned,
  grant-funded work.

## License

Apache-2.0 — see [`LICENSE`](LICENSE). SCF requires an open-source plan for smart
contracts; this contract is published under a permissive license as a reusable
open-source escrow pattern for rotating-savings / recurring cross-border payments.
