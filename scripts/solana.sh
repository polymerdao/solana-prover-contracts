# this script is to be sourced

SOLANA_VERSION='v2.1.21'

solana_installer_url="https://release.anza.xyz/$SOLANA_VERSION/install"

if ! command -v solana &>/dev/null; then
	echo "solana cli is not found. Will try to install it from $solana_installer_url"
	sh -c "$(curl -sSfL $solana_installer_url)"
	export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
fi
