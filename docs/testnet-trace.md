# Testnet trace: one complete 3-member, 3-round tanda

Network: **Stellar testnet**. Run on 2026-10-19 (UTC) with [`scripts/testnet-tanda-3x3.sh`](../scripts/testnet-tanda-3x3.sh). All transactions below were checked on Horizon testnet as `successful: true`. Testnet is periodically reset, so these links may stop resolving after a reset.

## Setup

| Item | Value |
|---|---|
| Contract (`tanda-escrow` v2) | [`CCHKHF37TL32QMXHSO7H54ER37N3GPVOS2PG5F75PSZNQ45TB3E3UZSB`](https://stellar.expert/explorer/testnet/contract/CCHKHF37TL32QMXHSO7H54ER37N3GPVOS2PG5F75PSZNQ45TB3E3UZSB) |
| Wasm hash (on-chain = local build) | `15b790175ea41bee16fc9af57e0d930d8c9466f0a16a2a78cfedadf760cc6b60` |
| Token | Circle **testnet** USDC `USDC:GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5`, SAC [`CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA`](https://stellar.expert/explorer/testnet/contract/CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA) |
| deployer | [`GBBHHM5XOINB2C5PGVZFEQCK7TQC5DE6TJEQOUTSD7CDQACSOVKXBMYY`](https://stellar.expert/explorer/testnet/account/GBBHHM5XOINB2C5PGVZFEQCK7TQC5DE6TJEQOUTSD7CDQACSOVKXBMYY) |
| m1 | [`GB2ONV4XAKUPUYZZCZF2PLG4BMS4B7SKUVKCMKIV5J3QYRNW3VT5GSIE`](https://stellar.expert/explorer/testnet/account/GB2ONV4XAKUPUYZZCZF2PLG4BMS4B7SKUVKCMKIV5J3QYRNW3VT5GSIE) |
| m2 | [`GC74REKINRQVWHPN2EBHJRXKCLTC7GFW45YL2IHTLKXQFVVO7RGSIJKM`](https://stellar.expert/explorer/testnet/account/GC74REKINRQVWHPN2EBHJRXKCLTC7GFW45YL2IHTLKXQFVVO7RGSIJKM) |
| m3 | [`GC2V7W272HJP4NGGVJEJS6HR6YVRU622ZKRJXDI2QF4EC6AURP3YZLZC`](https://stellar.expert/explorer/testnet/account/GC2V7W272HJP4NGGVJEJS6HR6YVRU622ZKRJXDI2QF4EC6AURP3YZLZC) |
| Group id | `0` |
| Rules | contribution 10 USDC (`100000000`), `start_ts` 1792443572, `round_secs` 90, `grace_secs` 60 |
| Payout order | round 0 → m3, round 1 → m1, round 2 → m2 |

All four accounts are throwaway testnet accounts funded by Friendbot. The USDC is real **testnet** USDC bought with testnet XLM on the testnet DEX (no mock token). Short rounds (90 s) were used only so the run fits in a few minutes.

## Transactions

| # | Step | Signed by | Transaction |
|---|---|---|---|
| 1 | USDC trustline | m1 | [`5ea0146b8693…`](https://stellar.expert/explorer/testnet/tx/5ea0146b8693ab6fa6ecae41fc840a076a25e8d03e6474b422f72e2f325449d2) |
| 2 | Buy 30 testnet USDC with XLM on the testnet DEX | m1 | [`c41ba543064d…`](https://stellar.expert/explorer/testnet/tx/c41ba543064d0bb171712082de7d3fe5bfda25ef9d16c931b384102134033f75) |
| 3 | USDC trustline | m2 | [`0263daa4b812…`](https://stellar.expert/explorer/testnet/tx/0263daa4b81270fe50b9c6236dfcc526dd8875653fb28409cb27709baeb66786) |
| 4 | Buy 30 testnet USDC with XLM on the testnet DEX | m2 | [`4e60979a215c…`](https://stellar.expert/explorer/testnet/tx/4e60979a215c302a6377933003f3118a8d90d77fbb17856eab3226aef2ca0015) |
| 5 | USDC trustline | m3 | [`4e55db6d016e…`](https://stellar.expert/explorer/testnet/tx/4e55db6d016eceb0b83828c997d2e340a648a58d632879efe5e985f0f00388e0) |
| 6 | Buy 30 testnet USDC with XLM on the testnet DEX | m3 | [`a194714d1a6e…`](https://stellar.expert/explorer/testnet/tx/a194714d1a6ea67250011d39e4b8f59215c53d59b0e7d15fd71d0b8ad5772c70) |
| 7 | Deploy `tanda-escrow` (create contract) | deployer | [`dd140e8ddeb2…`](https://stellar.expert/explorer/testnet/tx/dd140e8ddeb20f09004c7cd7c348f0491d21da4f638f24f65ea36aa60f498e23) |
| 8 | `create_group` (deployer is the creator, not a member) | deployer | [`6a6d9ffe2958…`](https://stellar.expert/explorer/testnet/tx/6a6d9ffe2958799731d125119092b04c65035fa437035e0d1900b213d2911507) |
| 9 | `contribute` round 0 | m1 | [`cd5dc402f404…`](https://stellar.expert/explorer/testnet/tx/cd5dc402f4041414654b73764a19cc66931a64d86977fd6e500742e6e3871aa9) |
| 10 | `contribute` round 0 | m2 | [`c39fd6433415…`](https://stellar.expert/explorer/testnet/tx/c39fd643341538810909de8eaf1d1153806d8057d7d38c234a46cc96afdbf9e3) |
| 11 | `contribute` round 0 | m3 | [`87fe7f70a823…`](https://stellar.expert/explorer/testnet/tx/87fe7f70a82369337878d118ba379040f8c48f6253d71cd99c69297f36f15309) |
| 12 | `release` round 0 | deployer | [`e8e9f5324d4f…`](https://stellar.expert/explorer/testnet/tx/e8e9f5324d4fc3ca0ebf8b547de8a23935f78d49938b3cd9fd6bc2089ef27a2b) |
| 13 | `contribute` round 1 | m1 | [`c792f2827e41…`](https://stellar.expert/explorer/testnet/tx/c792f2827e413823c539d62a4be83afbcde1e08af98cdd8690170e5ae3864464) |
| 14 | `contribute` round 1 | m2 | [`0ef9904ce745…`](https://stellar.expert/explorer/testnet/tx/0ef9904ce745a67c25211aba562316f92975fdcd53b17e4d8187b61c91167583) |
| 15 | `contribute` round 1 | m3 | [`a69ac80e5544…`](https://stellar.expert/explorer/testnet/tx/a69ac80e55449c4d7f910e5e23a2e1b7440c3644187754c53ba55751a1cbebf1) |
| 16 | `release` round 1 | m1 | [`992642810f40…`](https://stellar.expert/explorer/testnet/tx/992642810f40d8d078730cbf2d398d3456f9186fcc45f95e0ec57d043b7ae29c) |
| 17 | `contribute` round 2 | m1 | [`fb9bb58bbf50…`](https://stellar.expert/explorer/testnet/tx/fb9bb58bbf508867bead37e74dab68283cdd93dbf8c4f79834005723c2521920) |
| 18 | `contribute` round 2 | m2 | [`4777f48bc263…`](https://stellar.expert/explorer/testnet/tx/4777f48bc2638d73790258e580533500771535eba115dfa4b741727c8d5d1f76) |
| 19 | `contribute` round 2 | m3 | [`7ac116568903…`](https://stellar.expert/explorer/testnet/tx/7ac116568903590d9ce68cf6ec93bf687a7f268b866c25fc68a6991ebff2a9ba) |
| 20 | `release` round 2 | m3 | [`909908c2c9f2…`](https://stellar.expert/explorer/testnet/tx/909908c2c9f21ee00f44fe1dca54329dfbd3bba564d29f8d01df51f5c3c71bc3) |

Release callers were chosen on purpose: round 0 by the deployer (stands in for the La Tanda app, which has no special role), round 1 by m1 (its own beneficiary), round 2 by m3 (a member who is **not** that round's beneficiary). In every case the pot went to `payout_order[round]`.

## Checks that failed as expected (simulation, no transaction submitted)

| Check | Result |
|---|---|
| `release` round 0 before its due time | `Error(Contract, #7)` = `RoundNotDue` |
| `release` round 2 a second time | `Error(Contract, #9)` = `AlreadyReleased` |

## Final balances

| Account | USDC (stroops) |
|---|---|
| m1 | 300000000 (paid 3 × 10, received 30) |
| m2 | 300000000 (paid 3 × 10, received 30) |
| m3 | 300000000 (paid 3 × 10, received 30) |
| contract | 0 |

Not exercised on testnet (covered by unit tests only): the refund path for an incomplete round, and rejected attempts by non-members or by someone other than the member to move funds.

