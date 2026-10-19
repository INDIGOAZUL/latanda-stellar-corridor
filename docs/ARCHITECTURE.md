# La Tanda on Stellar: architecture (SCF #46 design)

Status: **design + testnet prototype**. The escrow contract described here is
deployed on **Stellar testnet only** and has **not been audited**. Nothing in this
document runs on mainnet today.

This document replaces the SCF #45 material in [`historical-scf45/`](historical-scf45/).
It describes one flow with one set of partners:

- **Ramp partner:** MoneyGram Ramps (cash-in and cash-out of Stellar USDC to and
  from the user's own wallet).
- **On-chain:** the `tanda-escrow` Soroban contract in
  [`contracts/tanda-escrow`](../contracts/tanda-escrow).
- **La Tanda:** the app (sign-up, reminders, indexer). It does not sign for users
  and does not hold their keys or their money.

No Bridge, no Stellar Disbursement Platform (SDP), no CCTP in this flow. Why each
one was dropped is explained in [Section 6](#6-what-changed-since-scf-45-and-why).

---

## 1. The tanda in one paragraph

A tanda (rotating savings group, ROSCA) has N members. Every round, every member
pays the same amount; the whole pot goes to one member; after N rounds each member
has received the pot exactly once. In `tanda-escrow` the members, the amount, the
payout order and the calendar are written on-chain when the group is created and
**cannot be changed afterwards**.

## 2. End-to-end flow

```mermaid
sequenceDiagram
    autonumber
    actor S as Member (US)
    participant MGin as MoneyGram Ramps<br/>(cash-in, US)
    participant WS as Member's own<br/>Stellar wallet
    participant APP as La Tanda app<br/>(no keys, no funds)
    participant C as tanda-escrow<br/>(Soroban contract)
    participant WB as Beneficiary's own<br/>Stellar wallet
    participant MGout as MoneyGram Ramps<br/>(cash-out)
    actor B as Beneficiary<br/>(e.g. El Salvador)

    Note over APP,C: Setup (once per group)
    APP->>C: create_group(members, amount, payout_order, start, round_secs, grace, USDC)<br/>signed by the group creator
    C-->>APP: event group_created (rules are now immutable)

    Note over S,C: Every round, every member
    S->>MGin: Pays cash at a MoneyGram location (KYC by MoneyGram)
    MGin->>WS: USDC sent to the member's own Stellar address
    APP-->>S: Reminder: round r is due on its due date
    S->>C: contribute(group, member, round)<br/>signed by the member in their wallet
    C->>C: pulls exactly `amount` USDC from the member's wallet
    C-->>APP: event contributed

    Note over C,WB: When the round is due and fully funded
    APP->>C: release(group, round)<br/>(any account may submit it; no signature required)
    C->>WB: pays the full pot to payout_order[round]
    C-->>APP: event released

    Note over WB,B: Cash-out (optional)
    B->>MGout: Starts a withdrawal from their wallet (KYC by MoneyGram)
    WB->>MGout: USDC sent by the beneficiary
    MGout->>B: Cash in local currency (USD in El Salvador) at a MoneyGram location

    Note over S,C: If the round is NOT fully funded by due + grace
    S->>C: refund(group, member, round)<br/>signed by that member: returns only their own contribution
    C-->>APP: event refunded
```

### 2.1 Entry: cash-in with MoneyGram Ramps

The member in the US buys USDC through **MoneyGram Ramps** (cash at a
MoneyGram location), and MoneyGram sends it **directly to the member's own Stellar
wallet**. The wallet runs MoneyGram's ramp flow; MoneyGram does the KYC/AML
for that transaction. La Tanda is not in the money path: it never receives the
fiat and never receives the USDC.

### 2.2 Contribution: the member signs `contribute`

This is the seam the SCF #45 panel could not follow (point 5). In #45 the design
had USDC minted via CCTP into a "corridor account" and then needed a `lock()` that
required the sender's signature, so it was unclear who signed what.

In this design there is **no corridor account**. The USDC is already in the
member's own wallet (2.1), and the member's wallet signs
`contribute(group, member, round)`. The contract then pulls exactly the
contribution amount from that wallet. The contract checks that:

- the signer is a member of the group (`member.require_auth()` + membership);
- the member has not already contributed to that round;
- the contribution window (due time + grace) has not closed.

### 2.3 Payout: `release` pays the beneficiary

`release(group, round)` **requires no signature and can be submitted by any
account**: the La Tanda app, the beneficiary, or anyone else. It succeeds only if
the round's due time has passed and every member has contributed, and it pays the
full pot to `payout_order[round]`, the beneficiary written at `create_group`. The
destination is not a parameter, so whoever submits the transaction cannot change
where the money goes. A round can be paid only once.

Note on the word "automatic": Stellar does not execute transactions on its own.
Someone has to submit `release`. What the contract guarantees is that **the
outcome does not depend on who submits it** and that **La Tanda's approval is not
needed**: if the La Tanda app stopped working, the beneficiary (or anyone) could
submit `release` directly.

The beneficiary can then keep the USDC in their wallet or cash out with
**MoneyGram Ramps**. El Salvador uses the US dollar, so no currency conversion is
needed there.

### 2.4 If a member does not pay

The contract does not force anyone to pay. If a round is not fully funded by its
due time plus the grace period (`grace_secs`, also fixed at `create_group`):

- the round can never be released;
- each member who did pay can call `refund(group, member, round)`, signed by
  themselves, and get **their own** contribution back;
- no one else (not La Tanda, not the group creator, not other members) can move
  that money.

A fully funded round can never be refunded; it can only be released to its
beneficiary. Recovering money from a member who stopped paying after receiving
their payout is an off-chain matter (social, or legal); the contract does not
solve it. This is the main credit risk of any tanda, on-chain or not.

## 3. Who can move money (custody, stated precisely)

| Actor | What it can do with funds held by the contract |
|---|---|
| Member | Contribute their own money (with their signature). Take back **their own** contribution from an incomplete round after due + grace. |
| Beneficiary of round r | Receives the pot of round r when it is released. |
| Any account (incl. La Tanda) | Submit `release` for a round that is due and complete. Cannot choose the destination. |
| Group creator | Nothing after `create_group`. |
| La Tanda | No admin role exists in the contract. Cannot withdraw, redirect, pause or sweep. |
| Contract upgrade | The contract has no upgrade function, so its code cannot be swapped. |

What La Tanda *does* control, honestly:

- The **app**: if La Tanda shows the wrong group or the wrong round, a member
  could be misled into signing. The member's wallet shows what they sign; the
  contract only accepts the exact contribution amount into this contract.
- **Group creation in the app**: La Tanda's app may create groups on behalf of
  coordinators. The rules are public on-chain from that moment and every
  member's contribution requires that member's own signature, so a member is not
  bound to a group they never paid into.

What MoneyGram controls: the fiat legs, their KYC/AML, limits, fees and
availability by country. La Tanda does not hold a money-transmission license and
does not perform fiat conversion.

We describe this as **"funds held by a contract with fixed rules and no admin"**.
We do not call it trustless: users still trust the Stellar network, the token
issuer (Circle for USDC), MoneyGram for the fiat legs, and an unaudited contract.

## 4. What stays off-chain and why

| Off-chain | Why |
|---|---|
| Profiles, identity, contacts | Personal data does not belong on a public ledger. |
| Group chat, reminders, notifications | High volume, private, no value in being on-chain. |
| Reputation / coordinator score | Built from behavior across groups; computed by the app from on-chain events plus app data. Not used by the contract. |
| Discovery (who forms a group with whom) | Social process; the result (members + order) goes on-chain at `create_group`. |
| KYC / AML | Done by MoneyGram for its cash-in and cash-out. |

The **indexer** (part of the La Tanda app) reads the four contract events
(`group_created`, `contributed`, `released`, `refunded`) to show round status,
send reminders and update reputation. It only reads; it has no keys.

La Tanda's existing product (rotating-savings groups in Honduras) runs on a
separate Cosmos app-chain and a Node.js/PostgreSQL backend. That product is not
wired to this contract today.

## 5. Risks and limits

- **No audit yet.** `tanda-escrow` has unit tests (21) and a testnet run, not an
  external audit. Do not use it with real money until it is audited.
- **Unpaid rounds.** See 2.4: the contract protects those who paid in an
  incomplete round, but cannot recover money from a member who defaults after
  receiving their payout.
- **MoneyGram dependency.** Cash-in/cash-out availability, KYC outcomes, limits
  and fees are MoneyGram's. Our MoneyGram Ramps integration is tested in
  **sandbox** only (one verified cash-out); refund, cash-in completion and
  webhooks are still to be tested; production access requires KYB.
- **Payout countries.** We have planned El Salvador (USD) as the first payout
  country because it uses the US dollar. MoneyGram Ramps cash-out there has not
  been confirmed in production yet. Payout to Honduras in lempiras
  through MoneyGram Ramps has **not** been verified by us.
- **Storage TTL.** Soroban persistent entries expire if not extended. The contract
  extends TTL on every write; a group untouched for months may need a restore
  operation before use.
- **Timing uses ledger timestamps** (seconds). Fine for rounds of days or weeks;
  not meant for sub-minute precision.
- **Wallet UX.** Members must manage their own Stellar wallet and keys. Losing
  keys means losing access to their own USDC; La Tanda cannot recover it.

## 6. What changed since SCF #45, and why

| SCF #45 | Now | Why |
|---|---|---|
| `release()` required the admin (La Tanda) signature; turns lived off-chain. | `release` needs no signature; members, order and calendar are on-chain and immutable. | Panel point 6: "non-custodial" was overstated. |
| CCTP mint into a corridor account, then `lock()` that needed the sender's signature. | Member receives USDC in their own wallet from MoneyGram and signs `contribute`. | Panel point 5: the seam was unclear. |
| CCTP for inbound USDC. | **Removed** from the flow. | MoneyGram delivers Stellar USDC directly; CCTP would only matter for a member who already holds USDC on another chain, which is that member's own action and not part of this design. |
| SDP paid beneficiaries. | **Removed.** The contract pays the beneficiary directly. | Panel point 5: "who pays the beneficiary, SDP or escrow?" Now only the escrow. |
| Bridge as off-ramp (Honduras). | **Removed.** MoneyGram Ramps for both ramps. | Bridge told us it does not support USDC on Stellar (and its pricing was high for us); a MoneyGram Ramps sandbox cash-out is already verified. |
| README, DEMO, architecture and STELLAR-INTEGRATION described different partners. | One document (this one) and a README that matches it. Old docs kept, marked historical. | Panel point 2. |

### SCF Integration List

The SCF Integration Track requires integrating at least one building block from
the official list. As of 2026-10-19 the
[SCF Handbook Integration List](https://stellar.gitbook.io/scf-handbook/scf-awards/build-award/integration-track/integration-list)
lists **"Moneygram"** under *On / Off-Ramping: Fiat Gateways & Anchor
Integrations* ("A global cash-in/cash-out partner enabling users to deposit and
withdraw funds at physical locations using Stellar rails"), linking to MoneyGram's
[Access to MoneyGram Ramps](https://developer.moneygram.com/moneygram-developer/docs/access-to-moneygram-ramps)
developer docs, with an estimated integration effort of "1+ month".

## 7. Contract interface (`tanda-escrow`)

| Function | Signature required | Effect |
|---|---|---|
| `create_group(creator, token, members, contribution, payout_order, start_ts, round_secs, grace_secs) -> group_id` | `creator` | Stores immutable rules. `payout_order` must be a permutation of `members`; >= 2 members; amount > 0. |
| `contribute(group_id, member, round)` | `member` | Transfers exactly `contribution` from the member to the contract. Once per member and round, before due + grace. |
| `release(group_id, round) -> amount` | none (anyone) | If due and fully funded and not yet paid: pays `contribution * members` to `payout_order[round]`. |
| `refund(group_id, member, round) -> amount` | `member` | If due + grace passed and the round is incomplete: returns that member's own contribution. |
| `get_group`, `get_round`, `get_contribution`, `round_due_ts` | none | Read-only views. |

Round `r` (0-based) is due at `start_ts + (r + 1) * round_secs`. Contributions are
accepted until `due + grace_secs`; refunds of an incomplete round open at that
same moment, so the two windows never overlap.

Events: `group_created`, `contributed`, `released`, `refunded` (topics include the
group id, and the round and address where relevant).

A testnet run of a full 3-member, 3-round tanda, with every transaction hash, is
in [`testnet-trace.md`](testnet-trace.md).
