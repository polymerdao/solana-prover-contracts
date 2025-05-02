#!/usr/bin/env bash

set -euo pipefail

# use this so the script can be run from anywhere
ROOT="$(realpath "$(dirname "$(realpath "$0")")"/..)"

main() {
	local releasedir="$ROOT/release"
	local targetdir="$ROOT/target/verifiable"
	rm -rf "$releasedir"
	mkdir -p "$releasedir"

	find "$targetdir" -type f -name "*.so" | while read -r so_file; do
		filename="$(basename "$so_file")"
		sha256sum "$so_file" >"$releasedir/${filename%.so}.sha256"
		cp "$so_file" "$releasedir"
	done

}

main "$@"
