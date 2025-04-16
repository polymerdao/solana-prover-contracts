#
# THIS SCRIPT IS ONLY MEANT TO BE EXECUTED BY OUR CI ACTION
#

# these weird paths make sense within the docker container
export PATH="$PATH:/root/goroot/bin:/root/gopath/bin"
export GOPATH=/root/gopath
export GOROOT=/root/goroot
export GOCACHE=/root/gocache
export GOMODCACHE=/root/gomodcache

# do this or the node_modules directory will be missing
yarn install --frozen-lockfile --prefer-offline

# anchor needs a key to deploy the program and sign transactions
solana-keygen new -s --no-bip39-passphrase --force

make test
make integration-test
make install-solana-anchor-go
make go-bindings

# allow the caching mechanism to read all these files
chmod -R o+rx ~/.cargo target
