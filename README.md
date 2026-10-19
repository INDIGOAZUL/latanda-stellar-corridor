# La Tanda on Stellar: tanda escrow

A **rotating-savings group (tanda) escrow** smart contract for Stellar (Soroban /
Rust). The group's members, contribution amount, payout order and calendar are
written on-chain when the group is created and cannot be changed afterwards.
Members pay in from their own wallets; each round's pot is paid to that round's
beneficiary by a `release` call that **anyone can submit and no admin approves**.
If a round is not fully funded, each member who paid can take back their own
contribution.

**Stellar testnet only. Not audited. No mainnet deployment.**

- Design and full flow: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- Testnet run of a complete 3-member, 3-round tanda: [`docs/testnet-trace.md`](docs/testnet-trace.md)
- Live La Tanda product: https://latanda.online
- Live product figures (canonical): https://latanda.online/api/public/metrics
- Founder GitHub: https://github.com/INDIGOAZUL

The live La Tanda product (rotating-savings groups in Honduras) runs today on a
**Cosmos** app-chain and a Node.js/PostgreSQL backend. This repo is a **separate
Stellar rail** and is not wired into that product yet.

---

## Status (Oct 2026)

- **`tanda-escrow` contract (v2):** written against `soroban-sdk 26.1`, 21 unit
  tests, deployed on **Stellar testnet** and exercised with a full 3x3 tanda
  using Circle's testnet USDC. Not audited, not on mainnet.
- **Ramp partner: MoneyGram Ramps** (cash-in and cash-out of Stellar USDC, to and
  from the user's own wallet; KYC/AML by MoneyGram). Integration tested in
  **sandbox** only; production access requires KYB. MoneyGram appears on the
  [SCF Integration List](https://stellar.gitbook.io/scf-handbook/scf-awards/build-award/integration-track/integration-list)
  as "Moneygram".
- **First payout country (planned):** El Salvador (USD). Payout to Honduras in
  lempiras through MoneyGram Ramps is not verified.
- **Stellar Community Fund #45:** La Tanda applied (Integration Track) and was
  **not selected**. The panel's feedback is reflected in this repo (see
  [ARCHITECTURE.md, section 6](docs/ARCHITECTURE.md#6-what-changed-since-scf-45-and-why)).
  The original application material is preserved under
  [`docs/historical-scf45/`](docs/historical-scf45/) and does **not** describe
  current plans.
- **Live product numbers (2026-09-09, from the endpoint above):** 80 registered
  users (not KYC-verified), 5 groups, 41 completed real-money payouts,
  L 1,652,100 HNL (≈ US$63,500) cycled. Those numbers describe the Cosmos-side
  product, not anything in this repo.

---

## How it works

```
 Member (US)                                           Beneficiary (e.g. El Salvador)
     │ cash/card                                                     ▲ cash (USD)
     ▼                                                               │
 MoneyGram Ramps ──USDC──► member's own wallet        beneficiary's own wallet ──USDC──► MoneyGram Ramps
  (cash-in, KYC)                  │                                ▲                (cash-out, KYC)
                                  │ contribute()                   │ release()
                                  │ signed by the member           │ any account may submit;
                                  ▼                                │ pays payout_order[round]
                       ┌──────────────────────────────────────────┴───┐
                       │ tanda-escrow (Soroban)                       │
                       │ immutable rules: members, amount, order,     │
                       │ calendar, grace. No admin.                   │
                       │ incomplete round after grace → refund() of   │
                       │ each member's own contribution               │
                       └──────────────────────────────────────────────┘

 La Tanda app: sign-up, reminders, indexer of contract events. Holds no keys, no funds.
```

### Who can move money

- **Members**: pay in their own money (signed), and take back their own
  contribution from an incomplete round after the grace period (signed).
- **Anyone**: can submit `release` for a round that is due and fully funded; the
  pot always goes to the beneficiary recorded for that round.
- **La Tanda / the group creator**: no admin role exists. They cannot withdraw,
  redirect, pause or sweep funds. The contract has no upgrade function.

We do not call this "trustless": users still rely on the Stellar network, the
USDC issuer, MoneyGram for the fiat legs, and an unaudited contract. Details and
risks: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

---

## Contract API (`contracts/tanda-escrow`)

| Function | Signature required | Effect |
|---|---|---|
| `create_group(creator, token, members, contribution, payout_order, start_ts, round_secs, grace_secs)` | `creator` | Stores immutable rules; returns `group_id`. |
| `contribute(group_id, member, round)` | `member` | Pulls exactly `contribution` from the member. Once per member and round. |
| `release(group_id, round)` | none | If due and fully funded: pays the pot to `payout_order[round]`. Once per round. |
| `refund(group_id, member, round)` | `member` | Incomplete round, after due + grace: returns that member's own contribution. |
| `get_group`, `get_round`, `get_contribution`, `round_due_ts` | none | Views. |

Events: `group_created`, `contributed`, `released`, `refunded`.

---

## Build, test, deploy (testnet)

Toolchain: **Rust** with the `wasm32v1-none` target, and the **Stellar CLI**.

```bash
# Unit tests (both contracts)
cargo test

# Optimized Wasm
cd contracts/tanda-escrow && stellar contract build
# -> target/wasm32v1-none/release/tanda_escrow.wasm

# Full 3-member / 3-round tanda on testnet (throwaway identities deployer, m1, m2, m3
# must exist in $STELLAR_CONFIG_HOME, funded with Friendbot)
STELLAR_CONFIG_HOME=$PWD/.stellar-testnet scripts/testnet-tanda-3x3.sh
```

---

## Repository layout

```
contracts/tanda-escrow/         # v2: the tanda escrow (current)
contracts/corridor-escrow-v1/   # v1: SCF #45 prototype, DEPRECATED (admin-triggered release)
scripts/testnet-tanda-3x3.sh    # end-to-end testnet run
docs/ARCHITECTURE.md            # design, flow, custody, risks
docs/testnet-trace.md           # transaction hashes of the testnet run
docs/historical-scf45/          # SCF #45 material, preserved, superseded
LICENSE                         # Apache-2.0
```

### Why v1 is kept (deprecated) instead of deleted

`corridor-escrow-v1` is the source of the earlier testnet deployment
`CBEJFGS23EI5MNJF4GAFHJSWDWXLIMAK2MWNWQI5H7VBTXRRUD6BA52U`, which is referenced in
the SCF #45 material and demo video. Keeping its source lets anyone check what
that deployment did (its `release` required the admin's signature) and compare
it with v2. It is not used by anything new.

---

## License

Apache-2.0, see [`LICENSE`](LICENSE).
