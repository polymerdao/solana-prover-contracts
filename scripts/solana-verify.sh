# this script is to be sourced

. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/solana-verify.env"

if ! command -v solana-verify &>/dev/null; then
	echo "solana-verify was not found. Will try to install it"
	cargo install solana-verify --version "$SOLANA_VERIFY_VERSION"
fi
