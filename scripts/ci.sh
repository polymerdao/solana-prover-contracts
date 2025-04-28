#!/usr/bin/env bash

set -euo pipefail

#
# THIS SCRIPT IS ONLY MEANT TO BE EXECUTED BY OUR CI ACTION
#

# do this or the node_modules directory will be missing
yarn install --frozen-lockfile --prefer-offline

# anchor needs a key to deploy the program and sign transactions when it runs the integration tests
# we can generate one with: solana-keygen new --no-bip39-passphrase --outfile ./foo
# let's hardcode one here to save us from installing the solana sdk
echo '[
	90,25,165,244,19,111,66,162,26,96,138,115,114,203,208,190,109,220,
	128,165,32,197,123,241,144,216,167,158,16,158,61,74,46,2,91,39,124,
	62,111,59,171,135,73,255,70,45,119,8,254,185,241,215,42,173,141,62,11,
	255,165,12,171,247,185,88
]' >/tmp/provider-wallet.json

make build
make test
make integration-test PROVIDER_WALLET=/tmp/provider-wallet.json
make install-solana-anchor-go
make go-bindings

# finding unstaged changes will mean something was not correctly commited, like the go bindings
if ! git diff --quiet; then
	echo "repo is modified!"
	git status --porcelain
	exit 1
fi

if [ -n "$VERIFIABLE" ]; then
	make build-verifiable
fi
