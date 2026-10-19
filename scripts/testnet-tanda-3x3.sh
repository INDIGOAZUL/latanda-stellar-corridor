#!/usr/bin/env bash
# Runs one complete 3-member / 3-round tanda against tanda-escrow on Stellar TESTNET.
# Identities (throwaway testnet keys) live in $STELLAR_CONFIG_HOME, which is
# git-ignored. This script never prints secret keys.
#
# Usage: STELLAR_CONFIG_HOME=<dir with deployer,m1,m2,m3> scripts/testnet-tanda-3x3.sh
# Output: one line per transaction in $LOG ("step<TAB>hash" or "step<TAB>ERROR ...").
set -uo pipefail

NET=testnet
USDC_ISSUER=GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5   # Circle testnet USDC
USDC="USDC:$USDC_ISSUER"
WASM=${WASM:-target/wasm32v1-none/release/tanda_escrow.wasm}
LOG=${LOG:-testnet-trace.log}
AMT=100000000          # 10 USDC (7 decimals)
ROUND_SECS=${ROUND_SECS:-90}
GRACE_SECS=${GRACE_SECS:-60}
: > "$LOG"

addr() { stellar keys address "$1"; }
D=$(addr deployer); M1=$(addr m1); M2=$(addr m2); M3=$(addr m3)

# run <step-name> <command...> : records the tx hash (64 hex) printed by the CLI.
run() {
  local step=$1; shift
  local out; out=$("$@" 2>&1); local rc=$?
  local h; h=$(printf '%s\n' "$out" | grep -oE '\b[0-9a-f]{64}\b' | tail -1)
  if [ $rc -eq 0 ]; then
    printf '%s\t%s\n' "$step" "${h:-no-hash}" | tee -a "$LOG"
  else
    printf '%s\tERROR %s\n' "$step" "$(printf '%s' "$out" | grep -iE 'error|Error\(' | head -2 | tr '\n' ' ')" | tee -a "$LOG"
  fi
  LAST_OUT=$out
}

echo "# accounts" | tee -a "$LOG"
printf 'deployer\t%s\nm1\t%s\nm2\t%s\nm3\t%s\n' "$D" "$M1" "$M2" "$M3" | tee -a "$LOG"

# 1. Members get testnet USDC: trustline + buy 30 USDC with XLM on the testnet DEX.
for m in m1 m2 m3; do
  run "trustline_$m" stellar tx new change-trust --source "$m" --line "$USDC" --network $NET
  run "buy_usdc_$m" stellar tx new path-payment-strict-receive --source "$m" \
      --send-asset native --send-max 600000000 --destination "$(addr $m)" \
      --dest-asset "$USDC" --dest-amount $((3*AMT)) --network $NET
done

# 2. USDC Stellar Asset Contract id (deploy it if not already deployed).
TOKEN=$(stellar contract id asset --asset "$USDC" --network $NET)
stellar contract asset deploy --asset "$USDC" --source deployer --network $NET >/dev/null 2>&1 || true
printf 'usdc_sac\t%s\n' "$TOKEN" | tee -a "$LOG"

# 3. Deploy the contract.
run deploy stellar contract deploy --wasm "$WASM" --source deployer --network $NET
CID=$(printf '%s\n' "$LAST_OUT" | grep -oE '\bC[A-Z2-7]{55}\b' | tail -1)
printf 'contract_id\t%s\n' "$CID" | tee -a "$LOG"

inv() { local step=$1 src=$2; shift 2; run "$step" stellar contract invoke --id "$CID" --source "$src" --network $NET -- "$@"; }

# 4. create_group, signed by the deployer (not a member). Payout order: m3, m1, m2.
START=$(date +%s)
inv create_group deployer create_group --creator "$D" --token "$TOKEN" \
  --members "[\"$M1\",\"$M2\",\"$M3\"]" --contribution "$AMT" \
  --payout_order "[\"$M3\",\"$M1\",\"$M2\"]" \
  --start_ts "$START" --round_secs "$ROUND_SECS" --grace_secs "$GRACE_SECS"
GID=$(printf '%s\n' "$LAST_OUT" | grep -oE '^[0-9]+$' | tail -1); GID=${GID:-0}
printf 'group_id\t%s\nstart_ts\t%s\n' "$GID" "$START" | tee -a "$LOG"

wait_until() { while [ "$(date +%s)" -lt "$1" ]; do sleep 5; done; sleep 10; }

# Release callers: round 0 = deployer (La Tanda app account), round 1 = m1
# (its own beneficiary), round 2 = m3 (a member who is NOT the beneficiary).
CALLERS=(deployer m1 m3)
for r in 0 1 2; do
  for m in m1 m2 m3; do
    inv "r${r}_contribute_$m" "$m" contribute --group_id "$GID" --member "$(addr $m)" --round "$r"
  done
  if [ $r -eq 0 ]; then
    # Negative check on-chain (simulation only, no tx): release before due must fail.
    inv "r0_release_before_due_expected_error" deployer release --group_id "$GID" --round 0
  fi
  wait_until $((START + (r+1)*ROUND_SECS))
  inv "r${r}_release_by_${CALLERS[$r]}" "${CALLERS[$r]}" release --group_id "$GID" --round "$r"
done

# Negative check: second release of round 2 must fail.
inv "r2_release_again_expected_error" deployer release --group_id "$GID" --round 2

echo "# final USDC balances (stroops)" | tee -a "$LOG"
for a in m1 m2 m3; do
  b=$(stellar contract invoke --id "$TOKEN" --source deployer --network $NET -- balance --id "$(addr $a)" 2>/dev/null)
  printf 'balance_%s\t%s\n' "$a" "$b" | tee -a "$LOG"
done
b=$(stellar contract invoke --id "$TOKEN" --source deployer --network $NET -- balance --id "$CID" 2>/dev/null)
printf 'balance_contract\t%s\n' "$b" | tee -a "$LOG"
