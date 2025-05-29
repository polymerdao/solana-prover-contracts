PROVIDER_WALLET := ~/.config/solana/id.json

.PHONY: build
build:
	anchor build

.PHONY: build-verifiable
build-verifiable:
	anchor build --verifiable --program-name polymer_prover

.PHONY: test
test:
	cargo test --workspace --locked -- --nocapture

.PHONY: integration-test
integration-test:
	anchor test --provider.wallet $(PROVIDER_WALLET)

.PHONY: go-bindings
go-bindings: build
	@if ! command -v solana-anchor-go > /dev/null; then \
		echo; \
		echo "TO INSTALL THE REQUIRED TOOLING RUN:"; \
		echo; \
		echo "  make install-solana-anchor-go"; \
		echo; \
		exit 0; \
	fi; \
	rm -rf ./go && \
	solana-anchor-go \
		--src=./target/idl/polymer_prover.json \
		--dst=./go \
		--mod github.com/polymerdao/solana-prover-contracts/go \
		--remove-account-suffix && \
	mkdir -p ./go/polymer-prover && \
	mv go/*.go ./go/polymer-prover && \
	solana-anchor-go \
		--src=./target/idl/mars.json \
		--dst=./go/mars \
		--remove-account-suffix && \
	cd ./go && \
	go mod tidy && \
	go test ./... -count=1


# install solana-anchor-go from a specific commit that includes our fixes
.PHONY: install-solana-anchor-go
install-solana-anchor-go:
	git clone --depth 1 --quiet https://github.com/fragmetric-labs/solana-anchor-go /tmp/$@ && \
	cd /tmp/$@ && \
	git checkout 2f389641e && \
	go install . && \
	rm -rf /tmp/$@ && \
	echo 'solana-anchor-go has been installed!'


# the weird argument is passed down to ts-mocha and it selects one test... there's no clean way of doing it
# so, this runs the before() step and the selected test, which means anchor will deploy our program and initialize it
# the --detach flag leaves the local node running
.PHONY: localnet
localnet:
	anchor test "\-\-grep 'internal accounts are set after init'" --detach


# this will be the new version. Use with `make upgrade-version VERSION=0.0.1`
VERSION :=

.PHONY: upgrade-version
upgrade-version: # integration-tests go-bindings
	@if [[ "$(VERSION)" == "" ]]; then echo "\n  usage: make upgrade-version VERSION=x.x.x\n"; exit 1; fi
	@./scripts/upgrade-version.sh $(VERSION)


TEST_VERSION := latest
.PHONY: test-deployment
test-deployment:
	export PROGRAM_KEYPAIR_FILE=./keypairs/polymer_prover-keypair.devnet.json; \
	export CLUSTER=localhost; \
	export VERSION=$(TEST_VERSION); \
	export TYPE=dev; \
	export KEYPAIR_FILE=~/.config/solana/id.json; \
	./scripts/deploy.sh
