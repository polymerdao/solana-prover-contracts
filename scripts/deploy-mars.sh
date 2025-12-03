#!/usr/bin/env bash

set -euo pipefail

# use this so the script can be run from anywhere
ROOT="$(realpath "$(dirname "$(realpath "$0")")"/..)"

check_env() {
	if [ -z "$POLYMER_ENV" ]; then
		echo "POLYMER_ENV env variable is not set" >&2
		exit 1
	fi
	if [ -z "$PROGRAM_KEYPAIR_FILE" ]; then
		echo "PROGRAM_KEYPAIR_FILE env variable is not set" >&2
		exit 1
	fi
}

main() {
	check_env

	# install the solana sdk
	. "$ROOT/scripts/solana.sh"
	# install solana-verify
	. "$ROOT/scripts/solana-verify.sh"

	local solana_cluster=''

	case "$POLYMER_ENV" in
	devnet | testnet) solana_cluster='devnet' ;;
	shadownet | mainnet) solana_cluster='mainnet-beta' ;;
	*)
		echo "unknown POLYMER_ENV '$POLYMER_ENV'" >&2
		exit 1
		;;
	esac

	echo "> using polymer environment '$POLYMER_ENV' and solana cluster '$solana_cluster'"

	# Build the verifiable program
	solana-verify build --library-name mars

	so_file="$ROOT/target/deploy/mars.so"

	if [ ! -f "$so_file" ]; then
		echo "artifact '$so_file' not found" >&2
		exit 1
	fi

	PROGRAM_ID="$(solana address -k "$PROGRAM_KEYPAIR_FILE")"

	echo "> deploying '$so_file' with program-id '$PROGRAM_ID' to solana cluster '$solana_cluster'"

	solana program deploy \
		"$so_file" \
		--url "$solana_cluster" \
		--program-id "$PROGRAM_KEYPAIR_FILE" \
		--commitment confirmed \
		--verbose

	echo "> verifying deployment of program-id '$PROGRAM_ID' on solana cluster '$solana_cluster'"

	args=(
		'--remote'
		'--url' "$solana_cluster"
		'--program-id' "$PROGRAM_ID"
		'--library-name' 'mars'
		'--skip-prompt'
		'--skip-build'
		'--current-dir'
		'https://github.com/polymerdao/solana-prover-contracts'
	)

	if ! solana-verify verify-from-repo "${args[@]}"; then
		echo "> could not verify program!" >&2
	fi

}

main "$@"
