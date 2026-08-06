# La Tanda — Stellar Integration Architecture (SCF #45, Integration Track)

**Project:** La Tanda — non-custodial USDC remittance & savings corridor for Latin America's underbanked.
**Scope of this document:** the Stellar-specific technical architecture and the concrete plan to integrate the chosen building blocks — **Circle CCTP**, the **Stellar Disbursement Platform (SDP)**, and a **licensed LATAM off-ramp (Bridge)** — orchestrated by an open-source **Soroban "Corridor Escrow."**

> Non-Stellar components (the existing Cosmos-SDK app-chain, the Node.js/PostgreSQL app) are described only where they touch the Stellar corridor. Everything funded by this grant is Stellar-native.

---

## 1. What we are building

Today a **cross-border tanda contribution is impossible** in the La Tanda product. This grant builds the corridor that makes it a first-class flow:

```
US sender ──(1) CCTP──▶ Stellar ──(2) Soroban Corridor Escrow──▶ (3) SDP payout ──▶ (4) Bridge off-ramp ──▶ HNL cash-out (Honduras)
             native USDC          non-custodial lock→release        bulk disbursement       USDC→fiat            unbanked recipient
```

A remitter in the US funds their family's tanda; the contribution rides into the corridor as **native USDC via CCTP**, is held **non-custodially** in a **Soroban escrow** until the beneficiary's turn, then paid out via the **Stellar Disbursement Platform** and cashed out to local currency through a **licensed off-ramp**. La Tanda never takes custody; the regulated fiat legs sit with the licensed off-ramp partner.

**Settlement asset:** native USDC. **No project token is issued or used in this corridor.**

---

## 2. Building blocks integrated (Integration List)

### 2.1 Circle CCTP — corridor inbound (native USDC cross-chain)
- **Role:** bring **native USDC** into the corridor via Circle's burn-and-mint Cross-Chain Transfer Protocol (e.g. Noble/other CCTP domains → Stellar), rather than depending on a single chain or wrapped/bridged USDC.
- **Integration work:** a backend service that (a) initiates/observes the source-chain burn, (b) fetches Circle's **attestation**, and (c) completes the **mint on Stellar** to the corridor's escrow account. Idempotent handling of attestations, retries, and reorg safety.
- **Why it matters:** the corridor is interoperable with the broader USDC economy from day one; a US sender's USDC can originate on the chain most convenient to them and still settle on Stellar.

### 2.2 Stellar Disbursement Platform (SDP) — tanda payouts
- **Role:** execute each tanda cycle's payout as a **bulk disbursement**. The tanda mechanic (each cycle, one—or at scale, many—beneficiaries receive their turn) maps directly onto SDP disbursements.
- **Integration work:** register the corridor's disbursement wallet with SDP; drive disbursements programmatically from the escrow-release event; consume SDP's **records, retries, and reporting** instead of hand-rolling payment orchestration. Reconciliation of SDP payment status back into the tanda ledger.
- **Why it matters:** production-grade cross-border payout rails (audit trail, retries, receipts) without building payment infrastructure from scratch.

### 2.3 Licensed LATAM off-ramp (Bridge) — last-mile cash-out
- **Role:** the **last mile** — convert USDC to local fiat (HNL) and deliver cash-out/deposit to the unbanked recipient in Honduras.
- **Integration work:** integrate the off-ramp partner's developer API (KYC handoff for the fiat leg, quote → payout → settlement callbacks). **Primary: Bridge (bridge.xyz)**; **alternates: Koywe, AlfredPay.** Final partner selection + developer-portal onboarding is a Tranche #0 deliverable; the corridor design requires no exclusive access and can route to more than one off-ramp.
- **Regulatory posture:** the **regulated fiat legs (KYC/AML, cash-out) sit entirely with the licensed off-ramp partner.** La Tanda is non-custodial and does not itself move fiat.

---

## 3. The Soroban Corridor Escrow (orchestrator)

The escrow is **the glue, not "the integration."** It is an open-source Soroban smart contract that holds a cross-border contribution **trustlessly** and releases the payout **on the tanda's schedule**, coordinating the three building blocks.

- **Status: already deployed and working on Stellar testnet.**
  - Contract: `CBEJFGS23EI5MNJF4GAFHJSWDWXLIMAK2MWNWQI5H7VBTXRRUD6BA52U`
  - `soroban-sdk 26.1.0`; `lock → release / refund` lifecycle with `cargo test` coverage.
  - Verifiable txs on stellar.expert: deploy/init, `lock`, `release` (see repo `DEMO.md`).
- **Flow:** receives the USDC that arrives via **CCTP** → holds it under the tanda's turn schedule → on the beneficiary's turn, emits a release that triggers the **SDP** disbursement toward the **off-ramp**. On failure/cancellation, `refund` returns funds to the contributor.
- **Grant work (v2):** wire the escrow to CCTP inbound and to SDP-triggered release, harden it for audit-readiness, and open-source the mainnet contract (Apache-2.0) in Tranche #3.

---

## 4. Supporting components

- **Cosmos↔Stellar indexer + settlement coordinator:** an off-chain indexer that reconciles the existing tanda (Cosmos-side product ledger) with its **Stellar USDC leg**, so the product's live tandas can adopt the corridor. Base design is an off-chain settlement coordinator; CCTP provides the native-USDC inbound, so no custom Cosmos↔Stellar token bridge is required.
- **Non-custodial wallet + USDC trustlines:** in-app wallet where users hold their own keys and USDC trustlines; the app never holds user funds.

---

## 5. End-to-end sequence

1. **Contribute:** US sender initiates a contribution → USDC enters the corridor via **CCTP** (burn on source domain → attestation → mint on Stellar to the escrow account).
2. **Hold:** the **Soroban Corridor Escrow** locks the USDC under the tanda's turn schedule (non-custodial; contributor retains refund rights until release conditions are met).
3. **Release:** at the beneficiary's turn, the escrow releases → triggers an **SDP** disbursement.
4. **Cash-out:** **SDP** pays the **off-ramp (Bridge)**, which converts USDC→HNL and delivers cash-out/deposit to the unbanked beneficiary in Honduras.
5. **Reconcile:** the indexer records SDP payment status and reconciles the Stellar leg back into the tanda ledger.

---

## 6. Security & trust model

- **Non-custodial throughout:** users hold their keys; escrow holds funds under on-chain, verifiable release/refund rules — never La Tanda operationally.
- **Regulated legs isolated:** KYC/AML and fiat cash-out are performed by the **licensed off-ramp partner**, not La Tanda.
- **Native USDC only:** settlement in native USDC (via CCTP) avoids wrapped-asset bridge risk. No project token.
- **Open source:** escrow + integration adapters (CCTP inbound, SDP payout, off-ramp, indexer) published Apache-2.0 as reusable ROSCA / recurring cross-border payment primitives for other LatAm builders.

---

## 7. Milestones (summary)

- **T#0 (upon approval):** KYC/KYB, public repo + board, testnet + CCTP environment, off-ramp developer-portal onboarding.
- **T#1 (testnet MVP):** CCTP inbound + Corridor Escrow v2 orchestrating it + non-custodial USDC wallet.
- **T#2 (testnet expansion):** SDP payout + Bridge off-ramp + Cosmos↔Stellar indexer + full recorded corridor demo.
- **T#3 (mainnet):** mainnet launch + Honduras pilot cohort + escrow/integration open-sourced + bilingual ES/EN docs.

---

## 8. Links

- Product: https://latanda.online
- Corridor repo (this repo): https://github.com/INDIGOAZUL/latanda-stellar-corridor (Apache-2.0)
- Testnet escrow contract: `CBEJFGS23EI5MNJF4GAFHJSWDWXLIMAK2MWNWQI5H7VBTXRRUD6BA52U` (lock/release txs on stellar.expert — see `DEMO.md`)
- Demo video (~2:52): https://youtu.be/izL3i273vhU
- Pitch deck (visual summary): [`docs/La-Tanda-SCF45-Deck-v2.pdf`](docs/La-Tanda-SCF45-Deck-v2.pdf)
