#!/usr/bin/env bash
#
# Build the OurDAO contract and deploy it to the Stellar testnet.
#
# Usage:
#   ./scripts/deploy-testnet.sh [identity-name]
#
# Environment overrides:
#   IDENTITY   stellar keys identity to deploy from   (default: ourdao-deployer)
#   NETWORK    network name to deploy to              (default: testnet)
#   ALIAS      local alias to save the contract id as (default: ourdao-dao)
#
# The identity is created and funded via friendbot automatically if it does
# not already exist. On success the deployed contract id is printed and saved
# under the given --alias for later `stellar contract invoke` calls.
set -euo pipefail

IDENTITY="${IDENTITY:-${1:-ourdao-deployer}}"
NETWORK="${NETWORK:-testnet}"
ALIAS="${ALIAS:-ourdao-dao}"
WASM_TARGET="wasm32v1-none"
WASM="target/${WASM_TARGET}/release/ourdao_dao.optimized.wasm"

# Run from the repo root regardless of where the script is invoked.
cd "$(dirname "$0")/.."

command -v stellar >/dev/null 2>&1 || {
  echo "error: the 'stellar' CLI is not installed. See https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli" >&2
  exit 1
}

echo "==> Ensuring identity '${IDENTITY}' exists and is funded on ${NETWORK}"
if ! stellar keys address "${IDENTITY}" >/dev/null 2>&1; then
  stellar keys generate "${IDENTITY}" --network "${NETWORK}" --fund
else
  # Already exists; top it up in case it is a fresh testnet reset.
  stellar keys fund "${IDENTITY}" --network "${NETWORK}" || true
fi
echo "    deployer address: $(stellar keys address "${IDENTITY}")"

echo "==> Building optimized wasm"
stellar contract build --optimize
ls -la "${WASM}"
WASM_SHA256=$(sha256sum "${WASM}" | awk '{print $1}')
echo "    SHA-256: ${WASM_SHA256}"

echo "==> Deploying to ${NETWORK}"
CONTRACT_ID=$(stellar contract deploy \
  --wasm "${WASM}" \
  --source "${IDENTITY}" \
  --network "${NETWORK}" \
  --alias "${ALIAS}")

echo ""
echo "==================================================================="
echo " Deployed OurDAO to ${NETWORK}"
echo "   contract id: ${CONTRACT_ID}"
echo "   saved alias: ${ALIAS}"
echo "   wasm sha256: ${WASM_SHA256}"
echo "==================================================================="
echo ""
echo "To verify this deployment against a release, see DEPLOYMENTS.md"
echo ""
echo "Next: initialize the DAO. Example (edit the values for your DAO):"
cat <<EOF

  stellar contract invoke --id ${CONTRACT_ID} --source ${IDENTITY} --network ${NETWORK} -- \\
    initialize \\
      --admins '["\$(stellar keys address ${IDENTITY})"]' \\
      --consensus_threshold 5100 \\
      --membership_fee 10000000 \\
      --token <TOKEN_CONTRACT_ID> \\
      --policy '{ "min_membership_duration": 0, "membership_contribution": 10000000, "max_loan_duration": 2592000, "min_interest_rate": 500, "max_interest_rate": 2000, "cooldown_period": 0, "max_loan_to_treasury_ratio": 5000, "default_grace_period": 259200, "default_penalty_bps": 2000 }'

Where <TOKEN_CONTRACT_ID> is the asset all DAO value flows through — e.g. the
testnet USDC contract, or the native XLM Stellar Asset Contract id from:

  stellar contract id asset --asset native --network ${NETWORK}
EOF
