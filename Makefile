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
	command -v solana-anchor-go > /dev/null || go install github.com/fragmetric-labs/solana-anchor-go@v1.0.2
	rm -rf ./go && \
	solana-anchor-go \
		--src=./target/idl/polymer_prover.json \
		--dst=./go \
		--mod github.com/polymerdao/solana-prover-contracts/go \
		--remove-account-suffix && \
	cd ./go && \
	go mod tidy && \
	go test .
