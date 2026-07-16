# DEMO — Corridor Escrow (Stellar testnet)

This walkthrough builds the contract, runs the tests, deploys to **Stellar
testnet**, and exercises the corridor lifecycle: **lock → release** (happy path)
and **lock → timeout → refund** (expiry path). It maps to SCF **Tranche #1**
("end-to-end testnet demo of a single-member cross-border contribution held and
released via Soroban") for the **US → El Salvador** pilot corridor.

> The **live product** demo is at **https://latanda.online** — that is the shipped
> Cosmos-side tanda product this Stellar rail plugs into. This DEMO covers only
> the net-new Stellar escrow contract.

---

## 0. Prerequisites

```bash
# Rust + Wasm target
rustup target add wasm32-unknown-unknown

# Stellar CLI (provides `stellar`, formerly `soroban`)
cargo install --locked stellar-cli

stellar --version
```

---

## 1. Test

```bash
# From the repo root
cargo test
```

Expected: all unit tests pass —

- `lock_then_release_pays_recipient` — sender locks USDC, coordinator releases to
  the pre-recorded recipient; balances move sender → escrow → recipient.
- `lock_then_timeout_refunds_sender` — after the expiry ledger, the sender
  self-refunds; balance returns to the sender.
- `refund_before_expiry_is_rejected` — refund before expiry → `NotYetExpired`.
- `refund_by_stranger_is_rejected` — a non-sender / non-admin cannot refund.
- `release_without_admin_auth_panics` — `release` requires the admin signature.
- `double_release_is_rejected`, `duplicate_escrow_id_is_rejected`,
  `zero_amount_is_rejected`, `double_initialize_is_rejected` — edge cases.

---

## 2. Build the Wasm

```bash
stellar contract build
# → target/wasm32-unknown-unknown/release/corridor_escrow.wasm
```

---

## 3. Identities & funding (testnet)

```bash
# Coordinator (admin), sender, recipient
stellar keys generate --global admin     --network testnet --fund
stellar keys generate --global sender    --network testnet --fund
stellar keys generate --global recipient --network testnet --fund

ADMIN=$(stellar keys address admin)
SENDER=$(stellar keys address sender)
RECIPIENT=$(stellar keys address recipient)
```

For the demo we use the **testnet USDC** issued by Circle's testnet issuer, or a
self-issued test asset. Its Stellar Asset Contract (SAC) address is our `token`.
Example using a self-issued `USDC` test asset:

```bash
# Deploy/lookup the SAC for the asset  USDC:<ISSUER_G...>
TOKEN=$(stellar contract asset id --asset USDC:$ADMIN --network testnet)
# (add a trustline + mint test USDC to $SENDER using classic Stellar ops or the SAC's mint)
```

> On mainnet, `token` is the **Circle-issued USDC** SAC address; no self-issuance.

---

## 4. Deploy & initialize

```bash
CID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/corridor_escrow.wasm \
  --source admin --network testnet)
echo "Contract: $CID"

stellar contract invoke --id $CID --source admin --network testnet -- \
  initialize --admin $ADMIN --token $TOKEN
```

---

## 5. Happy path: lock → release

```bash
# Sender locks 400 (7-dec units) USDC for the recipient, expiring at a future ledger.
stellar contract invoke --id $CID --source sender --network testnet -- \
  lock --sender $SENDER --escrow_id 1 --recipient $RECIPIENT \
       --amount 400 --expiry_ledger 5000000

# Inspect the escrow (status = Funded)
stellar contract invoke --id $CID --source sender --network testnet -- \
  get_escrow --escrow_id 1

# Coordinator releases to the pre-recorded recipient (status → Released)
stellar contract invoke --id $CID --source admin --network testnet -- \
  release --escrow_id 1
```

Verify the recipient's USDC balance increased by 400. This is the SCF Tranche #1
success signal: a cross-border contribution **held and released via Soroban**.

---

## 6. Expiry path: lock → timeout → refund

```bash
# Lock with a low expiry ledger so it is already/soon past expiry.
stellar contract invoke --id $CID --source sender --network testnet -- \
  lock --sender $SENDER --escrow_id 2 --recipient $RECIPIENT \
       --amount 250 --expiry_ledger 1

# Once the network ledger is >= expiry_ledger, the sender self-refunds.
stellar contract invoke --id $CID --source sender --network testnet -- \
  refund --caller $SENDER --escrow_id 2
```

Verify the sender's USDC balance is restored (status → Refunded).

---

## 7. Founder recording outline (screen recording + deck — founder's step)

Record a ~2–3 minute screen capture from the steps above:

1. **(0:00) Framing** — "La Tanda cross-border tanda corridor, US → El Salvador,
   non-custodial USDC escrow on Stellar. Live product at latanda.online." Show the
   site briefly.
2. **(0:20) Tests** — run `cargo test`, show all tests passing (lock→release,
   timeout→refund, unauthorized-access rejection).
3. **(0:45) Build & deploy** — `stellar contract build`, then deploy + initialize
   on testnet; show the contract id and an explorer link
   (`https://stellar.expert/explorer/testnet`).
4. **(1:20) Happy path** — run the `lock` then `release`; show the recipient
   balance change and the emitted events in the explorer.
5. **(2:00) Refund path** — run `lock` then `refund` after expiry; show funds
   return to the sender.
6. **(2:30) Close** — one slide on the Tranche #2 next step (SEP-24 anchor +
   MoneyGram Ramps SV cash-out) and the Cosmos↔Stellar settlement boundary.

Deck: 5–7 slides — problem (remittance cost / trapped tandas), solution
(non-custodial Stellar corridor), this contract, the SV-pilot → HN-expansion
sequencing, milestones/budget, team, links. Paste the repo URL + recording URL
into §11 of the SCF proposal before submitting.

---

## 8. Demo video

- **Demo video (~2:52):** https://youtu.be/izL3i273vhU — escrow live on Stellar testnet, product walkthrough, on-chain lock→release proof
- Full shot-by-shot storyboard: `stellar-demo-recording-frame.md` (La Tanda plans).
- On-chain references to show while recording:
  - Contract: https://stellar.expert/explorer/testnet/contract/CBEJFGS23EI5MNJF4GAFHJSWDWXLIMAK2MWNWQI5H7VBTXRRUD6BA52U
  - Lock tx: https://stellar.expert/explorer/testnet/tx/f6e615e09d003215c4bba44b4c94035870979f061e2bf5f507bfc722c716c813
  - Release tx: https://stellar.expert/explorer/testnet/tx/ffe1a4f76e9cc82557aa506d159d6a40a439faa056add5b861933bcbaaa30167
