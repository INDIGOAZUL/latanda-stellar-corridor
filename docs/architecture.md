# Corridor Architecture

How the Soroban **Corridor-Escrow** contract in this repo fits into La Tanda's
cross-border remittance rail on Stellar, the SEP / MoneyGram roles, the
Cosmos↔Stellar settlement boundary, and the El-Salvador-pilot → Honduras-expansion
sequencing.

> Honest scope: this is the **grant-funded v1 build**. The live La Tanda product
> (rotating-savings groups, ~80 registered/email-verified users, ~US$30K cycled)
> runs on a **Cosmos** app-chain today; there is **no Stellar integration in
> production yet**. Everything Stellar-side described here is the planned build the
> SCF award funds. Numbers describe the Cosmos product that de-risks the build; the
> Stellar layer is planned.

---

## 1. The problem this corridor solves

A *tanda* (ROSCA) is a rotating savings circle: a trusted group pools a fixed
contribution each cycle and each member takes the lump-sum payout in turn. Tandas
are how millions in Central America save — but they are **trapped locally**: a
member who migrates to the US cannot easily keep contributing to their family's
tanda, and payouts cannot reach a member abroad. Meanwhile remittances are the
lifeline (Honduras: ~US$9.74B in 2024, ~26% of GDP) yet cost ~6.36% on average and
often require a bank branch to cash out.

The corridor makes a **cross-border tanda contribution / payout** a first-class,
cheap, near-instant, cash-out-able flow using **USDC on Stellar** + **SEP anchors**
+ **MoneyGram Ramps** for the unbanked last mile.

---

## 2. Components and roles

```
  US sender ──(fiat cash-in)──► SEP-24 anchor / MoneyGram ──► USDC on Stellar
       │                                                          │
       │                                                    lock()│
       ▼                                                          ▼
  non-custodial                                       ┌───────────────────────┐
  Stellar wallet ─────────────────────────────────►  │ Corridor-Escrow (this │
  (user holds keys)                                   │ repo) — holds USDC per │
                                                      │ corridor rules only    │
                                                      └───────────┬───────────┘
                                                        release() │ / refund()
                                                                  ▼
  recipient / family ◄──(fiat cash-out)── MoneyGram Ramps SV ◄── USDC payout
  (unbanked, El Salvador)                 (SEP-24/31)            to recipient
```

### On-chain (this repo)
- **Corridor-Escrow (Soroban / Rust).** Non-custodial USDC escrow keyed by a
  corridor/payout id. `lock` (sender), `release` (admin → pre-recorded recipient),
  `refund` (sender/admin after expiry). Emits an event per state transition.

### Off-chain / integration (built in later tranches)
- **SEP-24 (interactive deposit/withdraw).** Hosted on/off-ramp with a regulated
  anchor: the **cash-in** leg (US sender's dollars → USDC) and the **cash-out** leg
  (USDC → local fiat) run through the anchor's SEP-24 flow.
- **SEP-31 (cross-border, institution-to-institution).** The institutional
  cross-border path for the US→SV corridor, extended toward US→HN where an anchor
  partner supports it.
- **MoneyGram Ramps / Access.** The physical **cash-in / cash-out** network for the
  *unbanked* member — walk in with cash / walk out with cash at an agent. Live for
  **El Salvador** today (via SEP-24), which is why the pilot corridor is US→SV.
- **Non-custodial Stellar wallet + USDC trustline** inside the La Tanda app: user
  holds their own keys; La Tanda never custodies funds.

---

## 3. Non-custodial guarantee

- Members hold their own Stellar keys. USDC moves **peer → escrow contract →
  peer** (beneficiary or anchor payout address).
- The escrow's destinations are **fixed by the sender at `lock` time**. The admin /
  coordinator can only *trigger* the pre-committed release or refund — it **cannot**
  redirect funds to an arbitrary address or sweep the balance. (See `release` /
  `refund` in `contracts/corridor-escrow/src/lib.rs`.)
- The **regulated money-transmission legs** (fiat cash-in / cash-out) are performed
  by the **licensed Stellar anchor / MoneyGram**, not by La Tanda and not by this
  contract. This is a deliberate design choice and a regulatory-risk mitigant that
  lets the corridor operate pre-Delaware-C-Corp.

---

## 4. Cosmos ↔ Stellar settlement boundary

La Tanda's savings-and-trust layer (tanda groups, contribution schedule, turn /
payout board, coordinator tooling, reputation) lives on the **Cosmos** app-chain
`latanda-testnet-1`. The **cross-border money movement** lives on **Stellar**
(USDC + this escrow + anchors + MoneyGram).

The boundary between them is a **settlement coordinator** (grant Tranche #2,
reference implementation):

- A tanda cycle event on Cosmos (a member's cross-border contribution is due, or a
  beneficiary's payout is scheduled) triggers a Stellar-side `lock` / `release` on
  this contract, and the emitted Stellar events are reflected back to the Cosmos
  ledger to keep the tanda's state consistent.
- Two candidate mechanisms (to be pinned down with SCF at Tranche #2): an
  **off-chain settlement coordinator** watching both chains' events, or an
  **IBC → Noble-USDC → Stellar** path. The escrow contract is agnostic to which is
  chosen — it exposes clean `lock` / `release` / `refund` primitives + events.

---

## 5. Sequencing — El Salvador pilot, Honduras expansion

Corridors are sequenced by **where rails already work**, to anchor delivery risk to
proven infrastructure:

- **US → El Salvador = the funded PILOT (Tranches #1–#2).** Rails are **confirmed**:
  MoneyGram Ramps USDC cash-out is **live** and USD is legal tender, so an
  end-to-end cash-out demo is de-risked. The **MVP (Tranche #1 — this contract on
  testnet)** and **testnet expansion (Tranche #2 — SEP-24 anchor + MoneyGram + SEP-31
  + settlement bridge)** are anchored here.
- **US → Honduras = the home-market EXPANSION (Tranche #3).** This is where La
  Tanda's ~80 existing users are (the impact base), but **MoneyGram Ramps is not
  confirmed in Honduras** — securing a HN cash-out/anchor partner is an **open
  dependency**, not a claimed capability. Targeted at Tranche #3, contingent on that
  partner. From there, template further Central American corridors (e.g. Guatemala).

---

## 6. Milestone mapping (SCF #45, Integration Track)

| Tranche | % | Deliverable (Stellar-integrated only) |
|---|---|---|
| #0 Acceptance | 10% | KYC; public repo (this) + board; testnet accounts provisioned. |
| #1 MVP testnet, US→SV | 20% | Non-custodial wallet + USDC trustline (testnet); **Corridor-Escrow v1 on testnet + tests**; CLI/demo of escrow → payout. |
| #2 Testnet expansion, US→SV | 30% | SEP-24 anchor + **MoneyGram Ramps** (SV, sandbox/testnet) + SEP-31 + Cosmos↔Stellar settlement reference; full recorded demo. |
| #3 Mainnet + HN expansion | 40% | Mainnet deploy; first real US→SV mainnet corridor tx; **secure a HN cash-out/anchor partner** + extend to HN users; bilingual docs. |

---

## References
- La Tanda product: https://latanda.online
- Stellar SEP-24 / SEP-31, MoneyGram Ramps, Circle USDC on Stellar — standard
  Stellar ecosystem building blocks (see the Stellar developer docs and the
  Stellar × MoneyGram case study).
