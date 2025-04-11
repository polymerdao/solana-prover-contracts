#!/usr/bin/env bash

set -euo pipefail

# set -x

# use this so the script can be run from anywhere
ROOT="$( realpath "$( dirname "$( realpath "$0" )" )"/.. )"
CARGO_FILE="$ROOT/programs/polymer-prover/Cargo.toml"

main() {
	readonly new_version="$1"
	if [[ ! "$new_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
		echo "$new_version is not SEMVER"
		exit 1
	fi

	local current_version
	current_version="$( awk -F'[= ]' '/^version/{ print $NF }' "$CARGO_FILE" | tr -d '"' )"

	echo "current version: $current_version"
	echo "new version:     $new_version"

	if [[ "$( printf '%s\n' "$current_version" "$new_version" | sort -V | tail -1 )" == "$current_version" ]]; then
		echo "new version is expected to be greater than current: $new_version < $current_version"
		exit 1
	fi

	readonly tag="v$new_version"

	if [[ "$( git tag --list "$tag" )" != "" ]]; then
		echo "tag $tag already exists. No changes have been made"
		exit 1
	fi

	# update the version in cargo file
	sed -i'' "/^version/ s/$current_version/$new_version/" "$CARGO_FILE"

	# commit the changes and tag the repo
	git add "$CARGO_FILE"
	git commit --quiet --message "upgrade version to $new_version"
	git tag "$tag"

	echo "version updated to $new_version!"

	git log -1 -p
}

main "$@"
