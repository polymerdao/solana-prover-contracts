#!/usr/bin/env bash

set -euo pipefail

# use this so the script can be run from anywhere
ROOT="$(realpath "$(dirname "$(realpath "$0")")"/..)"

check_env() {
	if [ -z "$PROGRAM_KEYPAIR_FILE" ]; then
		echo "PROGRAM_KEYPAIR_FILE env variable is not set" >&2
		exit 1
	fi
	if [ -z "$POLYMER_ENV" ]; then
		echo "POLYMER_ENV env variable is not set" >&2
		exit 1
	fi
	if [ -z "$TYPE" ]; then
		echo "TYPE env variable is not set" >&2
		exit 1
	fi
	if [ -z "$VERSION" ]; then
		echo "VERSION env variable is not set" >&2
		exit 1
	fi
	if [ -z "$KEYPAIR_FILE" ]; then
		echo "KEYPAIR_FILE env variable is not set" >&2
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

	local tag="$VERSION"
	if [ "$tag" = 'latest' ]; then
		tag="$(gh release list --limit 1 --json tagName --template '{{(index . 0).tagName}}')"
	fi

	echo "> downloading release artifacts for tag '$tag'"

	# this errors out if the $tag is not found causing the script to fail
	gh release download "$tag" \
		--repo polymerdao/solana-prover-contracts \
		--pattern 'polymer_prover.*.so' \
		--clobber \
		--dir "$ROOT/release"

	so_file="$ROOT/release/polymer_prover.${TYPE}.so"

	# Fail early if the artifact wasn't found
	if [ ! -f "$so_file" ]; then
		echo "artifact '$so_file' not found; did the build for TYPE='$TYPE' succeed?" >&2
		exit 1
	fi

	# calculate the program id from the provided keypair
	PROGRAM_ID="$(solana address -k "$PROGRAM_KEYPAIR_FILE")"

	echo "> deploying '$so_file' with program-id '$PROGRAM_ID' to solana cluster '$solana_cluster'"

	# finally, deploy the program
	solana program deploy \
		"$so_file" \
		--keypair "$KEYPAIR_FILE" \
		--url "$solana_cluster" \
		--program-id "$PROGRAM_KEYPAIR_FILE" \
		--commitment confirmed \
		--verbose

	deploy_dir="$ROOT/target/deploy"
	mkdir -p "$deploy_dir"
	cp "$so_file" "$deploy_dir/polymer_prover.so"

	echo "> verifying deployment of program-id '$PROGRAM_ID' on solana cluster '$solana_cluster'"

	args=(
		'--remote'
		'--url' "$solana_cluster"
		'--program-id' "$PROGRAM_ID"
		'--library-name' 'polymer_prover'
		'--skip-prompt'
		'--skip-build'
		'--current-dir'
		'--commit-hash' "$tag"
		'https://github.com/polymerdao/solana-prover-contracts'
	)

	if ! solana-verify verify-from-repo "${args[@]}"; then
		echo "> could not verify program!" >&2
	fi
}

main "$@"
