.PHONY: build
build:
	anchor build

.PHONY: test
test:
	cargo test -- --nocapture

.PHONY: integration-test
integration-test:
	anchor test

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
	cd ./go && \
	go mod tidy && \
	go test . -count=1

# for now we need to install our own fork of solana-anchor-go because theirs is broken. See diff for against
# upstream for the fixes we needed to put in place
.PHONY: install-solana-anchor-go
install-solana-anchor-go:
	git clone --branch v0.0.1 --depth 1 --quiet https://github.com/polymerdao/solana-anchor-go /tmp/$@ && \
	cd /tmp/$@ && \
	go install . && \
	rm -rf /tmp/$@ && \
	echo 'solana-anchor-go has been installed!'
