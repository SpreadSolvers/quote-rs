#!/usr/bin/env bash
# Builds a simple orderbook by quoting both ways (quote->base and base->quote).
# Amount is specified in quote tokens; base-side start uses amount_out of first quote->base step.
# Mid price = touch mid (step 0) from both directions.
# Usage: orderbook.sh <pool_id> <protocol> <quote_token> <base_token> <rpc_url> <start_amount_in> <increment_bps> <steps>
#   amounts in wei/smallest unit; start_amount_in = quote tokens; increment_bps = bps per step

set -euo pipefail

if [[ $# -ne 8 ]]; then
  echo "Usage: $0 <pool_id> <protocol> <quote_token> <base_token> <rpc_url> <start_amount_in> <increment_bps> <steps>"
  exit 1
fi

POOL_ID=$1
PROTOCOL=$2
QUOTE_TOKEN=$3
BASE_TOKEN=$4
RPC_URL=$5
START_AMOUNT_IN=$6
INCREMENT_BPS=$7
STEPS=$8

QUOTE_BIN=${QUOTE_BIN:-quote}
SCALE=18
BPS_SCALE=4

echo "Orderbook: pool=$POOL_ID protocol=$PROTOCOL steps=$STEPS"
echo "  quote_token -> base_token (amount in quote); base_token -> quote_token (start from first q->b amount_out)"
echo "Fetching quotes (quote -> base)..."
echo ""

# --- Side 1: quote -> base (amount_in in quote tokens)
declare -a Q2B_AMOUNTS_IN
declare -a Q2B_AMOUNTS_OUT

for (( i = 0; i < STEPS; i++ )); do
  AMOUNT_IN=$(echo "scale=0; (${START_AMOUNT_IN} * (10000 + ${i} * ${INCREMENT_BPS})) / 10000" | bc)
  AMOUNT_OUT=$("$QUOTE_BIN" "$POOL_ID" "$PROTOCOL" "$QUOTE_TOKEN" "$AMOUNT_IN" "$RPC_URL" 2>/dev/null) || true
  Q2B_AMOUNTS_IN+=("$AMOUNT_IN")
  Q2B_AMOUNTS_OUT+=("$AMOUNT_OUT")
done

# Start for base->quote = amount_out of first quote->base step
FIRST_BASE_OUT=${Q2B_AMOUNTS_OUT[0]:-0}
if [[ -z "$FIRST_BASE_OUT" || ! "$FIRST_BASE_OUT" =~ ^[0-9]+$ || "$FIRST_BASE_OUT" == "0" ]]; then
  echo "Warning: first quote->base returned no valid amount_out; base->quote side skipped."
  START_BASE_IN=0
else
  START_BASE_IN=$FIRST_BASE_OUT
fi

# --- Side 2: base -> quote (amount_in in base; start = first q->b amount_out)
echo "Fetching quotes (base -> quote)..."
echo ""

declare -a B2Q_AMOUNTS_IN
declare -a B2Q_AMOUNTS_OUT

for (( i = 0; i < STEPS; i++ )); do
  AMOUNT_IN=$(echo "scale=0; (${START_BASE_IN} * (10000 + ${i} * ${INCREMENT_BPS})) / 10000" | bc)
  AMOUNT_OUT=$("$QUOTE_BIN" "$POOL_ID" "$PROTOCOL" "$BASE_TOKEN" "$AMOUNT_IN" "$RPC_URL" 2>/dev/null) || true
  B2Q_AMOUNTS_IN+=("$AMOUNT_IN")
  B2Q_AMOUNTS_OUT+=("$AMOUNT_OUT")
done

# Touch mid (step 0, base per quote) from both sides
Q2B_IN_0=${Q2B_AMOUNTS_IN[0]}
Q2B_OUT_0=${Q2B_AMOUNTS_OUT[0]}
B2Q_IN_0=${B2Q_AMOUNTS_IN[0]}
B2Q_OUT_0=${B2Q_AMOUNTS_OUT[0]}
if [[ -n "$Q2B_OUT_0" && "$Q2B_OUT_0" =~ ^[0-9]+$ && "$Q2B_OUT_0" != "0" && "$Q2B_IN_0" != "0" ]]; then
  PRICE_Q2B=$(echo "scale=${SCALE}; ${Q2B_OUT_0} / ${Q2B_IN_0}" | bc)
else
  PRICE_Q2B="0"
fi
if [[ -n "$B2Q_OUT_0" && "$B2Q_OUT_0" =~ ^[0-9]+$ && "$B2Q_OUT_0" != "0" ]]; then
  PRICE_B2Q=$(echo "scale=${SCALE}; ${B2Q_IN_0} / ${B2Q_OUT_0}" | bc)
else
  PRICE_B2Q="0"
fi
if [[ "$PRICE_Q2B" != "0" && "$PRICE_B2Q" != "0" ]]; then
  MID_PRICE=$(echo "scale=${SCALE}; (${PRICE_Q2B} + ${PRICE_B2Q}) / 2" | bc)
elif [[ "$PRICE_Q2B" != "0" ]]; then
  MID_PRICE=$PRICE_Q2B
elif [[ "$PRICE_B2Q" != "0" ]]; then
  MID_PRICE=$PRICE_B2Q
else
  MID_PRICE="0"
fi

# CEX-style orderbook: asks (base->quote) on top, mid, then bids (quote->base) at bottom

# Table: base -> quote (asks, top)
echo "=== base -> quote (asks; amount_in = base, amount_out = quote) ==="
printf "%5s  %22s  %22s  %24s  %12s\n" "step" "amount_in(base)" "amount_out(quote)" "price(base/quote)" "vs_mid_bps"
printf "%5s  %22s  %22s  %24s  %12s\n" "----" "---------------" "-----------------" "-----------------" "---------"

for (( i = STEPS - 1; i >= 0; i-- )); do
  AMOUNT_IN=${B2Q_AMOUNTS_IN[$i]}
  AMOUNT_OUT=${B2Q_AMOUNTS_OUT[$i]}
  if [[ -z "$AMOUNT_OUT" || ! "$AMOUNT_OUT" =~ ^[0-9]+$ || "$AMOUNT_OUT" == "0" ]]; then
    printf "%5d  %22s  %22s  %24s  %12s\n" "$i" "$AMOUNT_IN" "${AMOUNT_OUT:-error}" "-" "-"
    continue
  fi
  PRICE=$(echo "scale=${SCALE}; ${AMOUNT_IN} / ${AMOUNT_OUT}" | bc)
  if [[ "$MID_PRICE" != "0" ]]; then
    VS_MID_BPS=$(echo "scale=${BPS_SCALE}; (${PRICE} - ${MID_PRICE}) / ${MID_PRICE} * 10000" | bc)
  else
    VS_MID_BPS="-"
  fi
  printf "%5d  %22s  %22s  %24s  %12s\n" "$i" "$AMOUNT_IN" "$AMOUNT_OUT" "$PRICE" "$VS_MID_BPS"
done

echo ""
echo "---"
echo "Mid price (touch, base per quote): ${MID_PRICE}"
echo "---"
echo ""

# Table: quote -> base (bids, bottom)
echo "=== quote -> base (bids; amount_in = quote, amount_out = base) ==="
printf "%5s  %22s  %22s  %24s  %12s\n" "step" "amount_in(quote)" "amount_out(base)" "price(base/quote)" "vs_mid_bps"
printf "%5s  %22s  %22s  %24s  %12s\n" "----" "----------------" "----------------" "-----------------" "---------"

for (( i = 0; i < STEPS; i++ )); do
  AMOUNT_IN=${Q2B_AMOUNTS_IN[$i]}
  AMOUNT_OUT=${Q2B_AMOUNTS_OUT[$i]}
  if [[ -z "$AMOUNT_OUT" || ! "$AMOUNT_OUT" =~ ^[0-9]+$ || "$AMOUNT_OUT" == "0" ]]; then
    printf "%5d  %22s  %22s  %24s  %12s\n" "$i" "$AMOUNT_IN" "${AMOUNT_OUT:-error}" "-" "-"
    continue
  fi
  PRICE=$(echo "scale=${SCALE}; ${AMOUNT_OUT} / ${AMOUNT_IN}" | bc)
  if [[ "$MID_PRICE" != "0" ]]; then
    VS_MID_BPS=$(echo "scale=${BPS_SCALE}; (${PRICE} - ${MID_PRICE}) / ${MID_PRICE} * 10000" | bc)
  else
    VS_MID_BPS="-"
  fi
  printf "%5d  %22s  %22s  %24s  %12s\n" "$i" "$AMOUNT_IN" "$AMOUNT_OUT" "$PRICE" "$VS_MID_BPS"
done

echo ""
echo "Done."
