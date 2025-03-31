# Solana Prover Contracts

[//]: # (TODO: add description)


## Install toolchain

The Solana program is written in Rust using the [Solana SDK](https://solana.com/docs) and the
[Anchor framework](https://www.anchor-lang.com/docs).
To install all dependencies simply follow [their installation guide](https://solana.com/docs/intro/installation).
It all boils down to this command:

```bash
curl --proto '=https' --tlsv1.2 -sSfL https://solana-install.solana.workers.dev | bash
```

> [!NOTE]
> Make sure the toolchain is added to your PATH:
> ```
> export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
> ```


Validate your setup running the following. You should not see errors or warnings

```bash
anchor build
```

## Running unit tests

We have a few `rust` unit tests inside `programs/polymer-prover/src/`. These cover only signature verification,
proof validation and event parsing without getting into the "solana program" or "anchor" aspect of the code.

```bash
cargo test
```

## Running integration tests

The integration tests make use of a local solana test validator node. `anchor` handles the whole process which is
roughly the following:
1. build program
2. start the local solana validator node
3. create accounts and fund them
4. deploy the program to the local node
5. run the tests

```bash
anchor test
```

## Solana test validator (localnet)

You can also use this validator node for extra testing (ie `proof-api`) simply by running

```bash
solana-test-validator
```

The node will be reachable at `http://127.0.0.1:8899`. Now you can use that node to deploy your program. To do so,
configure the `solana sdk` to point to your local env like so

```bash
solana config set --url localhost
```

Then just deploy your program

```bash
anchor deploy
```

Now we can run the same integration tests against the local node

```bash
anchor test --skip-local-validator --skip-deploy
```

But more importantly, youu can use the local node to validate proofs generated with the `proof-api` by pointing
the `e2e tool` to the local solana test validator node: `http://127.0.0.1:8899`.

## Solana devnet

You can also use the same tooling to deploy the program to `devnet`. First, configure the `solana sdk`

```bash
solana config set --url devnet
```

You will need a new account and funds, so run this

```bash
solana-keygen new
solana airdrop 2
```

Assuming the program is built (`anchor build`) you can now deploy it to devnet.
```bash
solana program deploy ./target/deploy/polymer_prover.so
```

Now, you can run the same tests against `devnet`
```bash
anchor test --skip-deploy --provider.cluster devnet
```
