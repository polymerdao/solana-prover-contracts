#!/usr/bin/env bash

set -eo pipefail

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
	if [ -z "$KEYPAIR_FILE" ]; then
		echo "KEYPAIR_FILE env variable is not set" >&2
		exit 1
	fi
}

main() {
	check_env

	# install the solana sdk
	. "$ROOT/scripts/solana.sh"

	# this takes care of checking if the cluster is valid
	solana config set --url "$CLUSTER"

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
