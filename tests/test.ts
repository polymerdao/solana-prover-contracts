import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PolymerProver } from "../target/types/polymer_prover";
import { assert } from "chai";
import * as fs from 'fs';
import {
  ComputeBudgetProgram,
} from "@solana/web3.js";

describe("initialize", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const user = provider.wallet as anchor.Wallet;
  const program = anchor.workspace.polymer_prover as Program<PolymerProver>;
  let eventAccount = anchor.web3.Keypair.generate();

  it("Is initialized!", async () => {
    const clientType = "proof_api";
    const signerAddress = Buffer.from('8D3921B96A3815F403Fb3a4c7fF525969d16f9E0', 'hex');
    console.log(signerAddress);
    const peptideChainId = new anchor.BN(901);

    const tx = await program.methods.initialize(clientType, signerAddress, peptideChainId)
      .accounts({
        eventAccount: eventAccount.publicKey,
        user: user.publicKey,
      })
      .signers([eventAccount])
      .rpc();
    console.log("TxHash ::", tx);

    const account = await program.account.eventAccount.fetch(eventAccount.publicKey);
    assert.equal(account.clientType, clientType)
    assert.deepEqual(Buffer.from(account.signerAddr), signerAddress)
    assert.equal(account.peptideChainId.toNumber(), peptideChainId.toNumber())
  });

  it("validate event", async () => {
    const content = fs.readFileSync('programs/polymer-prover/src/instructions/test-data/op-proof-v2.hex', 'utf-8');
    const proof = Buffer.from(content.trim().slice(2), 'hex');

    const tx = await program.methods.validateEvent(proof)
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
      .accounts({
        eventAccount: eventAccount.publicKey,
      })
      .rpc();
    console.log("TxHash ::", tx);

  });


});
