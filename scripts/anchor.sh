# this script is to be sourced

if [ -z "$ANCHOR_VERSION" ]; then
	echo "ANCHOR_VERSION env variable is not set" >&2
	exit 1
fi

if ! command -v avm &>/dev/null; then
	echo 'avm is not found. Will install it'
	cargo install --git https://github.com/coral-xyz/anchor avm --force
fi

avm install "$ANCHOR_VERSION"
avm use "$ANCHOR_VERSION"
