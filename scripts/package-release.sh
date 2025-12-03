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

}

main "$@"
