#!/usr/bin/env bash

set -euo pipefail

# use this so the script can be run from anywhere
ROOT="$(realpath "$(dirname "$(realpath "$0")")"/..)"

check_env() {
	if [ -z "$PROGRAM_KEYPAIR" ]; then
		echo "PROGRAM_KEYPAIR env variable is not set" >&2
		exit 1
	fi
	if [ -z "$TYPE" ]; then
		echo "TYPE env variable is not set" >&2
		exit 1
	fi
}

main() {
	check_env

	. "$ROOT/scripts/solana.sh"

	# calculate the program id from the provided keypair
	ID="$(solana address -k - <<<"$PROGRAM_KEYPAIR")"

	# since anchor wants the program id as part of the actual binary, let's add it here
	# use perl to make the substitution portable across macOS and ubuntu
	perl -pi -e "s/^declare_id!\(\"[^\"]*\"\);/declare_id!(\"$ID\");/" "$ROOT/programs/polymer-prover/src/lib.rs"

	echo "> building verifiable program with ID $ID"

	# build the binary and rename it according to the type. This should be dev or main
	make build-verifiable
	mv "$ROOT/target/verifiable/polymer_prover.so" "$ROOT/target/verifiable/polymer_prover.${TYPE}.so"

	# clean up the new id
	git checkout -- "$ROOT/programs/polymer-prover/src/lib.rs"
}

main "$@"
