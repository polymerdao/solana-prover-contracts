import * as anchor from "@coral-xyz/anchor";
import * as fs from 'fs';
import { Program } from "@coral-xyz/anchor";
import { PolymerProver } from "../target/types/polymer_prover";
import { assert } from "chai";
import { ComputeBudgetProgram, ConfirmOptions, VersionedTransactionResponse } from "@solana/web3.js";

describe("initialize", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const user = provider.wallet as anchor.Wallet;
  const program = anchor.workspace.polymer_prover as Program<PolymerProver>;
  let eventAccount = anchor.web3.Keypair.generate();

  const confirmOptions: ConfirmOptions = { commitment: "confirmed" };

  it("Is initialized!", async () => {
    const clientType = "proof_api";
    const signerAddress = Buffer.from('8D3921B96A3815F403Fb3a4c7fF525969d16f9E0', 'hex');
    const peptideChainId = new anchor.BN(901);

    const tx = await program.methods.initialize(clientType, signerAddress, peptideChainId)
      .accounts({
        eventAccount: eventAccount.publicKey,
        user: user.publicKey,
      })
      .signers([eventAccount])
      .rpc(confirmOptions);
    console.log("TxHash ::", tx);

    const account = await program.account.eventAccount.fetch(eventAccount.publicKey);
    assert.equal(account.clientType, clientType)
    assert.deepEqual(Buffer.from(account.signerAddr), signerAddress)
    assert.equal(account.peptideChainId.toNumber(), peptideChainId.toNumber())
  });

  it("validate event", async () => {
    const content = fs.readFileSync('programs/polymer-prover/src/instructions/test-data/op-proof-small.hex', 'utf-8');
    const proof = Buffer.from(content.trim().slice(2), 'hex');

    console.log(`proof size ${proof.length}`);
    const txHash = await program.methods.validateEvent(proof)
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
      .accounts({
        eventAccount: eventAccount.publicKey,
      })
      .rpc(confirmOptions);
    console.log("TxHash ::", txHash);

    const tx = await provider.connection.getTransaction(txHash, {
      maxSupportedTransactionVersion: 0,
      commitment: "confirmed",
    });

    // TODO https://solana.stackexchange.com/questions/3463/how-to-parse-event-in-transaction-log-with-anchor ??
    const buffer = getReturnLogData(tx);

    const expectedEvent = JSON.parse(
      fs.readFileSync('programs/polymer-prover/src/instructions/test-data/op-event-small.json', 'utf-8')
    );

    const result = new anchor.BorshCoder(program.rawIdl).types.decode('ValidateEventResult', buffer);

    let topics = Buffer.alloc(0);
    expectedEvent.topics.map((topic: string) => topics = Buffer.concat([topics, Buffer.from(topic.slice(2), 'hex')]))

    assert.equal(84_532, result.chain_id);
    assert.equal(expectedEvent.address.slice(2), Buffer.from(result.emitting_contract).toString('hex'));
    assert.equal(Buffer.from(topics).toString('hex'), Buffer.from(result.topics).toString('hex'));
    assert.equal(expectedEvent.data.slice(2), Buffer.from(result.unindexed_data).toString('hex'));
  });
});

const getReturnLogData = (tx: VersionedTransactionResponse): Buffer<ArrayBuffer> => {
  const prefix = "Program return: ";
  let log = tx.meta.logMessages.find((log) => log.startsWith(prefix));
  log = log.slice(prefix.length);
  const [_key, data] = log.split(" ", 2);
  return Buffer.from(data, "base64");
};
