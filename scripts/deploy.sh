#!/usr/bin/env bash

set -eo pipefail

KEYPAIR_FILE="$(mktemp)"

# always remove the keypair to avoid leaks
trap 'rm -rf $KEYPAIR_FILE' EXIT

SOLANA_INSTALLER_URL='https://release.anza.xyz/v2.1.21/install'

# use this so the script can be run from anywhere
ROOT="$(realpath "$(dirname "$(realpath "$0")")"/..)"

check_env() {
	if [ -z "$PROGRAM_ID" ]; then
		echo "PROGRAM_ID env variable is not set" >&2
		exit 1
	fi
	if [ -z "$CLUSTER" ]; then
		echo "CLUSTER env variable is not set" >&2
		exit 1
	fi
	if [ -z "$VERSION" ]; then
		echo "VERSION env variable is not set" >&2
		exit 1
	fi
	if [ -z "$KEYPAIR" ]; then
		echo "KEYPAIR env variable is not set" >&2
		exit 1
	fi
}

main() {
	check_env

	if ! command -v solana &>/dev/null; then
		echo "solana cli is not found. Will try to install it from $SOLANA_INSTALLER_URL"
		sh -c "$(curl -sSfL $SOLANA_INSTALLER_URL)"
		export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
	fi

	# this takes care of checking if the cluster is valid
	solana config set --url "$CLUSTER"

	echo -n "$KEYPAIR" >"$KEYPAIR_FILE"
	solana config set --keypair "$KEYPAIR_FILE"

	# the release downloader fetches the latest release by default but the tag must be empty
	# using 'latest' will not work
	local tag=''
	if [ "$VERSION" != 'latest' ]; then
		tag="$VERSION"
	fi

	# this errors out if the $tag is not found causing the script to fail
	gh release download "$tag" \
		--repo polymerdao/solana-prover-contracts \
		--pattern polymer_prover.so \
		--clobber \
		--output polymer_prover.so

	if ! solana program show "$PROGRAM_ID" &>/dev/null; then
		# program does not exist in the cluster, deploy it for the first time. For which we need the
		# program keypair instead of the program id
		echo "> deploying program '$PROGRAM_ID' to cluster '$CLUSTER' for the first time"
		PROGRAM_ID="$ROOT/keypair/polymer_prover-keypair.json"
	fi

	# finally, deploy the program
	solana program deploy \
		./polymer_prover.so \
		--program-id "$PROGRAM_ID" \
		--commitment confirmed \
		--verbose
}

main "$@"
