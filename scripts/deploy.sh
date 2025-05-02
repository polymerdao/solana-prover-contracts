#!/usr/bin/env bash

set -eo pipefail

# use this so the script can be run from anywhere
ROOT="$(realpath "$(dirname "$(realpath "$0")")"/..)"

check_env() {
	if [ -z "$PROGRAM_KEYPAIR_FILE" ]; then
		echo "PROGRAM_KEYPAIR_FILE env variable is not set" >&2
		exit 1
	fi
	if [ -z "$CLUSTER" ]; then
		echo "CLUSTER env variable is not set" >&2
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
		--pattern 'polymer_prover.*.so' \
		--clobber \
		--dir "$ROOT/release"

	so_file="$ROOT/release/polymer_prover.${TYPE}.so"

	# Fail early if the artifact wasn’t found
	if [ ! -f "$so_file" ]; then
		echo "artefact '$so_file' not found; did the build for TYPE='$TYPE' succeed?" >&2
		exit 1
	fi

	echo "> deploying '$so_file' to '$CLUSTER'"

	# finally, deploy the program
	solana program deploy \
		"$so_file" \
		--program-id "$PROGRAM_KEYPAIR_FILE" \
		--commitment confirmed \
		--verbose
}

main "$@"
