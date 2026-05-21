#!/usr/bin/env bash

set -euo pipefail

# use this so the script can be run from anywhere
ROOT="$(realpath "$(dirname "$(realpath "$0")")"/..)"

main() {
	local release_dir="$ROOT/release"
	local target_dir="$ROOT/target/deploy"
	rm -rf "$release_dir"
	mkdir -p "$release_dir"

	find "$target_dir" -type f -name "*.so" | while read -r so_file; do
		filename="$(basename "$so_file")"
		sha256sum "$so_file" >"$release_dir/${filename%.so}.sha256"
		cp "$so_file" "$release_dir"
	done

	# include the IDL so `anchor idl init/upgrade` can be run during deploy.
	# the IDL is produced by `anchor build` (via ci.sh's `make build`).
	local idl_src="$ROOT/target/idl/polymer_prover.json"
	if [ -f "$idl_src" ]; then
		cp "$idl_src" "$release_dir/polymer_prover.idl.json"
	fi
}

main "$@"
