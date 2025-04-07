import * as anchor from "@coral-xyz/anchor";
import * as fs from 'fs';
import * as path from "path";
import { Program } from "@coral-xyz/anchor";
import { PolymerProver } from "../target/types/polymer_prover";
import { assert } from "chai";
import {
  ComputeBudgetProgram,
  ConfirmOptions,
  VersionedTransactionResponse,
  Keypair,
  SendTransactionError,
  PublicKey,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";

describe("initialize", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.polymer_prover as Program<PolymerProver>;

  const wallet = provider.wallet as anchor.Wallet;
  const confirmOptions: ConfirmOptions = { commitment: "confirmed" };
  const testDataPath = 'programs/polymer-prover/src/instructions/test-data';
  const proof = readProofFile('op-proof-small.hex')

  before(async () => {
    console.log(`PROGRAM ID: ${program.programId}`)
    console.log(`WALLET:     ${wallet.publicKey.toBase58()}`)
  })

  it("initialize", async () => {
    const clientType = "proof_api";
    const signerAddress = Array.from(Buffer.from('8D3921B96A3815F403Fb3a4c7fF525969d16f9E0', 'hex'));
    const peptideChainId = new anchor.BN(901);

    const signature = await program.methods.initialize(clientType, signerAddress, peptideChainId)
      .accounts({ authority: wallet.publicKey })
      .signers([wallet.payer])
      .rpc(confirmOptions);

    await provider.connection.getTransaction(signature, {
      maxSupportedTransactionVersion: 0,
      commitment: "confirmed",
    });

    const [pda, _bump] = PublicKey.findProgramAddressSync([Buffer.from("internal")], program.programId);
    const account = await program.account.internalAccount.fetch(pda);
    assert.equal(account.clientType, clientType)
    assert.deepEqual(account.signerAddr, signerAddress)
    assert.equal(account.peptideChainId.toNumber(), peptideChainId.toNumber())
  });

  // see https://solana.com/developers/courses/program-security/reinitialization-attacks#summary
  it("fails to re-initialize  with different authority", async () => {
    const newOwner = await generateAndFundNewSigner()

    try {
      const sig = await program.methods.initialize("", Array.from([0]), new anchor.BN(1))
        .accounts({ authority: newOwner.publicKey })
        .signers([newOwner])
        .rpc(confirmOptions);

      const tx = await provider.connection.getTransaction(sig, {
        maxSupportedTransactionVersion: 0,
        commitment: "confirmed",
      });
      console.log(tx);
      throw new Error("re-initialize should have failed");
    }
    catch (err: any) {
      assert.ok(err instanceof SendTransactionError)
      const txerr = err as SendTransactionError
      assert.ok(txerr.logs !== undefined)
      assert.ok(txerr.logs.find((log: string) => log.endsWith('already in use')))
    }
  });

  // happy path to validate event. The instruction is called by a new user (different from the program's deployer)
  // it checks that the program accepts proofs in chunks and temporarily stores them in a PDA account.
  // Once all the chunks have been sent, it runs the actual event validation
  it("validates event", async () => {
    const newSigner = wallet.payer // await generateAndFundNewSigner()

    await program.methods
      .validateEvent(proof.subarray(0, 800), proof.length)
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions);

    // at this point the first chunk should be stored in the cache pda
    const [cachePda, _bump] = PublicKey.findProgramAddressSync([newSigner.publicKey.toBuffer()], program.programId);
    const cache0 = await program.account.proofCacheAccount.fetch(cachePda, "confirmed")
    assert.deepEqual(proof.subarray(0, 800), cache0.cache)

    const signature = await program.methods
      .validateEvent(proof.subarray(800), proof.length)
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions);

    // now that we have sent the second chunk which completes the proof, the cache must be empty
    const cache1 = await program.account.proofCacheAccount.fetch(cachePda, "confirmed")
    assert.equal(0, cache1.cache.length)

    const txs = await provider.connection.getTransactions([signature], {
      maxSupportedTransactionVersion: 0,
      commitment: "confirmed",
    });

    checkValidatEventResult(84_532, 'op-event-small.json', ...txs)
  });


  // same as previous one but with two concurrent users. The proof is the same in both cases but it is split in
  // different chunks for every user
  it("validates events from two users at the same time", async () => {
    const user0 = await generateAndFundNewSigner()
    const user1 = await generateAndFundNewSigner()

    await Promise.all([
      program.methods
        .validateEvent(proof.subarray(0, 800), proof.length)
        .accounts({ authority: user0.publicKey })
        .signers([user0])
        .rpc(confirmOptions),
      program.methods
        .validateEvent(proof.subarray(0, 700), proof.length)
        .accounts({ authority: user1.publicKey })
        .signers([user1])
        .rpc(confirmOptions)
    ])

    // at this point the first chunks should be stored in their respective pda accounts
    const [cachePda0, _bump0] = PublicKey.findProgramAddressSync([user0.publicKey.toBuffer()], program.programId);
    const [cachePda1, _bump1] = PublicKey.findProgramAddressSync([user1.publicKey.toBuffer()], program.programId);

    {
      const caches = await Promise.all([
        program.account.proofCacheAccount.fetch(cachePda0, "confirmed"),
        program.account.proofCacheAccount.fetch(cachePda1, "confirmed"),
      ])
      assert.deepEqual(proof.subarray(0, 800), caches[0].cache)
      assert.deepEqual(proof.subarray(0, 700), caches[1].cache)
    }

    // send the second chunks now
    const signatures: string[] = await Promise.all([
      program.methods
        .validateEvent(proof.subarray(800), proof.length)
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
        .accounts({ authority: user0.publicKey })
        .signers([user0])
        .rpc(confirmOptions),
      program.methods
        .validateEvent(proof.subarray(700), proof.length)
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
        .accounts({ authority: user1.publicKey })
        .signers([user1])
        .rpc(confirmOptions)
    ])

    // now that we have sent all the chunks, both caches must be empty
    {
      const caches = await Promise.all([
        program.account.proofCacheAccount.fetch(cachePda0, "confirmed"),
        program.account.proofCacheAccount.fetch(cachePda1, "confirmed"),
      ])
      assert.equal(0, caches[0].cache.length)
      assert.equal(0, caches[1].cache.length)
    }

    const txs = await provider.connection.getTransactions(signatures, {
      maxSupportedTransactionVersion: 0,
      commitment: "confirmed",
    });

    checkValidatEventResult(84_532, 'op-event-small.json', txs.at(-2))
    checkValidatEventResult(84_532, 'op-event-small.json', txs.at(-1))
  });

  // calls validate event with invalid input, causing the PDA account to be closed and then calls it again with
  // valid input to verify the program could recover
  it("recovers from an error", async () => {
    const newSigner = await generateAndFundNewSigner()

    // this will cause the PDA account to be closed since the second chunk wille make the cache
    // going beyond the total proof size
    const sigs0: string[] = await Promise.all([
      program.methods
        .validateEvent(proof.subarray(0, 700), proof.length)
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
        .accounts({ authority: newSigner.publicKey })
        .signers([newSigner])
        .rpc(confirmOptions),
      program.methods
        .validateEvent(proof.subarray(0, 701), proof.length)
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
        .accounts({ authority: newSigner.publicKey })
        .signers([newSigner])
        .rpc(confirmOptions)
    ])

    const txs0 = await provider.connection.getTransactions(sigs0, {
      maxSupportedTransactionVersion: 0,
      commitment: "confirmed",
    });

    // since we are sending two txs concurrently, we don't really know which one will be procesed first. We do know
    // that the second one will cause this error log to be returned.
    assert.ok(txs0.find((tx: VersionedTransactionResponse) => tx.meta.logMessages.find((log: string) =>
      log.endsWith(`invalid proof cache len 1401 > ${proof.length}`)
    )))

    // at this point the PDA cache account should be closed, so calling valide event again with valid inputs should
    // works as expected

    await program.methods
      .validateEvent(proof.subarray(0, 800), proof.length)
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions);

    const signature1 = await program.methods
      .validateEvent(proof.subarray(800), proof.length)
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions);
    const txs = await provider.connection.getTransactions([signature1], {
      maxSupportedTransactionVersion: 0,
      commitment: "confirmed",
    });

    checkValidatEventResult(84_532, 'op-event-small.json', ...txs)
  });


  it.skip("errors out if cache account limit is reached ", async () => {
    //     Error: AnchorError caused by account: cache_account. Error Code: AccountDidNotSerialize. Error Number: 3004. Error Message: Failed to serialize the account.
  })

  function checkValidatEventResult(chainId: number, eventFileName: string, ...txs: VersionedTransactionResponse[]) {
    const result = findEvent('validateEventEvent', txs)
    assert.ok(result)

    let topics = Buffer.alloc(0);
    const expectedEvent = readEventFile(eventFileName)
    expectedEvent.topics.map((topic: string) => topics = Buffer.concat([topics, Buffer.from(topic.slice(2), 'hex')]))

    assert.equal(chainId, result.chainId);
    assert.equal(expectedEvent.address.slice(2), Buffer.from(result.emittingContract).toString('hex'));
    assert.equal(Buffer.from(topics).toString('hex'), Buffer.from(result.topics).toString('hex'));
    assert.equal(expectedEvent.data.slice(2), Buffer.from(result.unindexedData).toString('hex'));
  }

  function readProofFile(fileName: string): Buffer<ArrayBuffer> {
    const content = fs.readFileSync(path.join(testDataPath, fileName), 'utf-8');
    return Buffer.from(content.trim().slice(2), 'hex');
  }

  function readEventFile(fileName: string): any {
    return JSON.parse(fs.readFileSync(path.join(testDataPath, fileName), 'utf-8'))
  }

  async function generateAndFundNewSigner(): Promise<Keypair> {
    const signer = Keypair.generate();
    const latestBlockhash = await provider.connection.getLatestBlockhash();

    const signature = await provider.connection.requestAirdrop(signer.publicKey, 2 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction({
      signature,
      blockhash: latestBlockhash.blockhash,
      lastValidBlockHeight: latestBlockhash.lastValidBlockHeight,
    })
    return signer
  }

  function findEvent(name: string, txs: VersionedTransactionResponse[]): any {
    const eventParser = new anchor.EventParser(program.programId, program.coder);
    for (const tx of txs) {
      if (tx.meta === undefined || tx.meta.logMessages == undefined) continue
      for (const event of eventParser.parseLogs(tx.meta.logMessages)) {
        if (event.name === name) return event.data
      }
    }
    throw Error(`event ${name} not found`)
  }

  // const MAX_TRANSACTION_SIZE = 1232;
  //
  // async function calculteMaxProofSizePerTx(signer: Keypair): Promise<number> {
  //   const block = await provider.connection.getLatestBlockhash()
  //   const tx = new Transaction({
  //     blockhash: block.blockhash,
  //     lastValidBlockHeight: block.lastValidBlockHeight,
  //     feePayer: signer.publicKey,
  //   });
  //   tx.add(await program.methods
  //     .validateEvent(Buffer.from([]), 0)
  //     .accounts({ authority: signer.publicKey })
  //     .instruction())
  //   tx.sign(signer)
  //
  //   const overhead = tx.serialize().length + 1
  //   console.log(`overhead: ${overhead}`)
  //   return MAX_TRANSACTION_SIZE - overhead
  // }
  //
  //  here's a way to send the tx without using anchor bs which seems to add extra stuff
  //   const instruction = await program.methods
  //     .validateEvent(proof.subarray(0, proof.length - 10), proof.length)
  //     .accounts({ authority: newSigner.publicKey })
  //     .signers([newSigner])
  //     .instruction()
  //   const block = await provider.connection.getLatestBlockhash()
  //   const tx = new Transaction({
  //     blockhash: block.blockhash,
  //     lastValidBlockHeight: block.lastValidBlockHeight,
  //     feePayer: newSigner.publicKey,
  //   });
  //   tx.add(instruction)
  //   tx.sign(newSigner)
  //
  //   console.log(`tx size ${tx.serialize().length}`);
  //   const signatures = [await sendAndConfirmTransaction(provider.connection, tx, [newSigner], confirmOptions)]
  //
  //
  //   signatures.push(
  //     await program.methods
  //       .validateEvent(proof.subarray(proof.length - 10), proof.length)
  //       .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
  //       .accounts({ authority: newSigner.publicKey })
  //       .signers([newSigner])
  //       .rpc(confirmOptions)
  //   )
  //
  //
  //   const rxs = await provider.connection.getTransactions(signatures, {
  //     maxSupportedTransactionVersion: 0,
  //     commitment: "confirmed",
  //   });
});
