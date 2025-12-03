# this script is to be sourced

SOLANA_VERIFY_VERSION='0.4.11'

if ! command -v solana-verify &>/dev/null; then
	echo "solana-verify was not found. Will try to install it"
	cargo install solana-verify --version "$SOLANA_VERIFY_VERSION"
fi
