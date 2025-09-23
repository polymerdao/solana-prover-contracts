import * as anchor from '@coral-xyz/anchor'
import * as fs from 'fs'
import * as path from 'path'
import { Program } from '@coral-xyz/anchor'
import type { PolymerProver } from '../target/types/polymer_prover'
import type { CpiClient } from '../target/types/cpi_client'
import type { Mars } from '../target/types/mars'
import {
  ComputeBudgetProgram,
  type ConfirmOptions,
  type VersionedTransactionResponse,
  Keypair,
  SendTransactionError,
  PublicKey,
  LAMPORTS_PER_SOL,
} from '@solana/web3.js'
import bs58 from 'bs58'
import { execSync } from 'child_process'
import { describe, it, beforeAll, expect } from 'bun:test'

describe('localnet', () => {
  const provider = anchor.AnchorProvider.env()
  anchor.setProvider(provider)
  const program = anchor.workspace.polymer_prover as Program<PolymerProver>
  const cpiclient = anchor.workspace.cpi_client as Program<CpiClient>
  const mars = anchor.workspace.mars as Program<Mars>

  const wallet = provider.wallet as anchor.Wallet
  const confirmOptions: ConfirmOptions = { commitment: 'confirmed' }
  const testDataPath = 'programs/polymer-prover/src/instructions/test-data'
  const proof = readProofFile('op-proof-v2.hex')

  const clientType = 'proof_api'
  const signerAddress = Buffer.from('8D3921B96A3815F403Fb3a4c7fF525969d16f9E0', 'hex')
  const peptideChainId = new anchor.BN(901)

  const programKeypairFile: string = process.env.PROGRAM_KEYPAIR_FILE ?? 'target/deploy/polymer_prover-keypair.json'

  beforeAll(async () => {
    console.log(`WALLET:            ${wallet.publicKey.toBase58()}`)
    console.log(`POLYMER PROVER ID: ${program.programId}`)
    console.log(`CPI CLIENT ID:     ${cpiclient.programId}`)
    console.log(`MARS ID:           ${mars.programId}`)

    const out0 = runProverCtl(
      '--keypair',
      bs58.encode(wallet.payer.secretKey),
      'initialize',
      '--signer-addr',
      signerAddress.toString('hex'),
      '--client-type',
      'proof_api',
      '--peptide-chain-id',
      peptideChainId.toString(),
    )
    expect(out0.includes('Instruction: Initialize'))

    const out1 = runProverCtl('--keypair', bs58.encode(wallet.payer.secretKey), 'create-accounts')
    expect(out1.includes('accounts successfully created'))
  })

  it('internal accounts are set after init', async () => {
    const pda = findProgramAddress([Buffer.from('internal')], program.programId)
    const account = await program.account.internalAccount.fetch(pda)
    expect(account.clientType).toEqual(clientType)
    expect(account.signerAddr).toEqual(Array.from(signerAddress))
    expect(account.peptideChainId.toNumber()).toEqual(peptideChainId.toNumber())
  })

  // see https://solana.com/developers/courses/program-security/reinitialization-attacks#summary
  it('fails to re-initialize with different authority', async () => {
    const newOwner = await generateAndFundNewSigner()
    const programKey = Uint8Array.from(JSON.parse(fs.readFileSync(programKeypairFile, { encoding: 'utf-8' })))
    const programPair = Keypair.fromSecretKey(programKey)

    try {
      const sig = await program.methods
        .initialize('', Array.from([0]), new anchor.BN(1))
        .accounts({ authority: newOwner.publicKey })
        .signers([newOwner, programPair])
        .rpc(confirmOptions)

      const tx = await provider.connection.getTransaction(sig, {
        maxSupportedTransactionVersion: 0,
        commitment: 'confirmed',
      })
      console.log(tx)
      throw new Error('re-initialize should have failed')
    } catch (err: any) {
      expect(err instanceof SendTransactionError)
      const txerr = err as SendTransactionError
      expect(txerr.logs !== undefined)
      expect(txerr.logs!.find((log: string) => log.endsWith('already in use')))
    }
  })

  // happy path to validate event. The instruction is called by a new user (different from the program's deployer)
  // it checks that the program accepts proofs in chunks and temporarily stores them in a PDA account.
  // Once all the chunks have been sent, it runs the actual event validation
  it('validates event', async () => {
    const newSigner = await generateAndFundNewSigner()

    await program.methods
      .loadProof(proof.subarray(0, 800))
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions)

    // at this point the first chunk should be stored in the cache pda
    const cachePda = findProgramAddress([Buffer.from('cache'), newSigner.publicKey.toBuffer()], program.programId)
    const cache0 = await program.account.proofCacheAccount.fetch(cachePda, 'confirmed')
    expect(Buffer.from(proof.subarray(0, 800))).toStrictEqual(Buffer.from(cache0.cache))

    await program.methods
      .loadProof(proof.subarray(800))
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions)

    // cache must be full at this point
    const cache1 = await program.account.proofCacheAccount.fetch(cachePda, 'confirmed')
    expect(Buffer.from(proof)).toStrictEqual(Buffer.from(cache1.cache))

    // now run the actual validation
    const signature = await program.methods
      .validateEvent()
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions)

    const txs = await provider.connection.getTransactions([signature], {
      maxSupportedTransactionVersion: 0,
      commitment: 'confirmed',
    })
    expect(txs.length).toBeGreaterThan(0)

    // once the validation runs, the cache must be empty
    const cache2 = await program.account.proofCacheAccount.fetch(cachePda, 'confirmed')
    expect(cache2.cache.length).toEqual(0)

    txs.forEach((t) => {
      if (t) console.log(t.meta?.logMessages ?? [])
    })

    expect(findLogMessage('proof is valid', ...txs.filter((t) => t !== null))).toBeDefined()
    await checkValidatationResult(newSigner, 11_155_420, 'op-event-v2.json')
  })

  it('closes accounts', async () => {
    const newSigner = await generateAndFundNewSigner()
    const cachePda = findProgramAddress([Buffer.from('cache'), newSigner.publicKey.toBuffer()], program.programId)
    const resultPda = findProgramAddress([Buffer.from('result'), newSigner.publicKey.toBuffer()], program.programId)

    // close the cache and result account now
    const lamportsBefore = await provider.connection.getBalance(newSigner.publicKey, 'confirmed')

    const out = runProverCtl('--keypair', bs58.encode(newSigner.secretKey), 'close-accounts')
    expect(out.includes('accounts successfully closed'))

    const lamportsAfter = await provider.connection.getBalance(newSigner.publicKey, 'confirmed')
    expect(lamportsAfter).toBeGreaterThan(lamportsBefore) // Receiver should get lamports back

    const cache = await provider.connection.getAccountInfo(cachePda, 'confirmed')
    expect(cache).toBeNull() // cache account should be closed

    const result = await provider.connection.getAccountInfo(resultPda, 'confirmed')
    expect(result).toBeNull() // result account should be closed

    // now try to create accounts again
    await program.methods
      .createAccounts()
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions)

    const cache1 = await provider.connection.getAccountInfo(cachePda, 'confirmed')
    expect(cache1).toBeDefined() // cache account should be created again

    const result1 = await provider.connection.getAccountInfo(resultPda, 'confirmed')
    expect(result1).toBeDefined() // result account should be created again
  })

  it('validates event with large proof', async () => {
    const largeProof = readProofFile('arb-proof-v2.hex')
    console.log(`large proof size: ${largeProof.length}`)
    console.log(`proof size: ${proof.length}`)

    const newSigner = await generateAndFundNewSigner()

    // loop through the large proof and send it in chunks
    const maxProofSize = Math.min(largeProof.length, 800)
    for (let start = 0; start < largeProof.length; start += maxProofSize) {
      const end = Math.min(start + maxProofSize, largeProof.length)
      console.log(`loading proof chunk: ${start} - ${end}`)
      await program.methods
        .loadProof(largeProof.subarray(start, end))
        .accounts({ authority: newSigner.publicKey })
        .signers([newSigner])
        .rpc(confirmOptions)
    }

    // now run the actual validation
    const signature = await program.methods
      .validateEvent()
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions)

    const txs = await provider.connection.getTransactions([signature], {
      maxSupportedTransactionVersion: 0,
      commitment: 'confirmed',
    })

    txs.forEach((t) => console.log(t?.meta?.logMessages ?? []))

    checkValidatationResult(newSigner, 421614, 'arb-event-v2.json')
  })

  // same as previous one but with two concurrent users. The proof is the same in both cases but it is split in
  // different chunks for every user
  it('validates events from two users at the same time', async () => {
    const user0 = await generateAndFundNewSigner()
    const user1 = await generateAndFundNewSigner()

    await Promise.all([
      program.methods
        .loadProof(proof.subarray(0, 800))
        .accounts({ authority: user0.publicKey })
        .signers([user0])
        .rpc(confirmOptions),
      program.methods
        .loadProof(proof.subarray(0, 700))
        .accounts({ authority: user1.publicKey })
        .signers([user1])
        .rpc(confirmOptions),
    ])

    // at this point the first chunks should be stored in their respective pda accounts
    const cachePda0 = findProgramAddress([Buffer.from('cache'), user0.publicKey.toBuffer()], program.programId)
    const cachePda1 = findProgramAddress([Buffer.from('cache'), user1.publicKey.toBuffer()], program.programId)

    {
      const caches = await Promise.all([
        program.account.proofCacheAccount.fetch(cachePda0, 'confirmed'),
        program.account.proofCacheAccount.fetch(cachePda1, 'confirmed'),
      ])
      expect(Buffer.from(proof.subarray(0, 800))).toStrictEqual(Buffer.from(caches[0].cache))
      expect(Buffer.from(proof.subarray(0, 700))).toStrictEqual(Buffer.from(caches[1].cache))
    }

    // send the second chunks now
    await Promise.all([
      program.methods
        .loadProof(proof.subarray(800))
        .accounts({ authority: user0.publicKey })
        .signers([user0])
        .rpc(confirmOptions),
      program.methods
        .loadProof(proof.subarray(700))
        .accounts({ authority: user1.publicKey })
        .signers([user1])
        .rpc(confirmOptions),
    ])

    // now that we have sent all the chunks, both caches must be full
    {
      const caches = await Promise.all([
        program.account.proofCacheAccount.fetch(cachePda0, 'confirmed'),
        program.account.proofCacheAccount.fetch(cachePda1, 'confirmed'),
      ])
      expect(Buffer.from(proof)).toStrictEqual(Buffer.from(caches[0].cache))
      expect(Buffer.from(proof)).toStrictEqual(Buffer.from(caches[1].cache))
    }

    // run the validations now
    const signatures: string[] = await Promise.all([
      program.methods
        .validateEvent()
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
        .accounts({ authority: user0.publicKey })
        .signers([user0])
        .rpc(confirmOptions),
      program.methods
        .validateEvent()
        .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
        .accounts({ authority: user1.publicKey })
        .signers([user1])
        .rpc(confirmOptions),
    ])

    const txs = await provider.connection.getTransactions(signatures, {
      maxSupportedTransactionVersion: 0,
      commitment: 'confirmed',
    })

    {
      const caches = await Promise.all([
        program.account.proofCacheAccount.fetch(cachePda0, 'confirmed'),
        program.account.proofCacheAccount.fetch(cachePda1, 'confirmed'),
      ])
      expect(caches[0].cache.length).toEqual(0)
      expect(caches[1].cache.length).toEqual(0)
    }

    txs.forEach((t) => console.log(t?.meta?.logMessages ?? []))

    expect(findLogMessage('proof is valid', txs.at(-2)!)).toBeDefined()
    checkValidatationResult(user0, 11_155_420, 'op-event-v2.json')

    expect(findLogMessage('proof is valid', txs.at(-1)!)).toBeDefined()
    checkValidatationResult(user1, 11_155_420, 'op-event-v2.json')
  })

  // calls validate event with invalid input, causing the cache to be cleared and then calls it again with
  // valid input to verify the program could recover
  it('recovers from an error', async () => {
    const newSigner = await generateAndFundNewSigner()

    await program.methods
      .loadProof(proof.subarray(0, 700))
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions)

    const sig0 = await program.methods
      .validateEvent()
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions)

    const txs0 = await provider.connection.getTransactions([sig0], {
      maxSupportedTransactionVersion: 0,
      commitment: 'confirmed',
    })

    txs0.forEach((t) => console.log(t?.meta?.logMessages ?? []))

    // since we are sending two an incomplete proof, we expect the event validation to fail like so
    expect(
      txs0.find((tx) =>
        tx?.meta?.logMessages?.find((log: string) => log.endsWith("invalid membership proof: can't read path")),
      ),
    ).toBeDefined()

    // make sure the result account contains the expected error
    const resultAccount = findProgramAddress([Buffer.from('result'), newSigner.publicKey.toBuffer()], program.programId)
    const result = await program.account.validationResultAccount.fetch(resultAccount, 'confirmed')
    expect(result.isValid).toBe(false)
    expect(result.errorMessage).toEqual("invalid membership proof: can't read path")
    expect(result.chainId).toEqual(0)
    expect(Buffer.from(result.emittingContract).toString('hex')).toEqual('0000000000000000000000000000000000000000')
    expect(result.topics.length).toEqual(0)
    expect(result.unindexedData.length).toEqual(0)

    // at this point the PDA cache account should be clear, so calling valide event again with valid inputs should
    // works as expected
    await program.methods
      .loadProof(proof.subarray(0, 700))
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions)

    await program.methods
      .loadProof(proof.subarray(700))
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions)

    const signature1 = await program.methods
      .validateEvent()
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions)
    const txs = await provider.connection.getTransactions([signature1], {
      maxSupportedTransactionVersion: 0,
      commitment: 'confirmed',
    })

    txs.forEach((t) => console.log(t?.meta?.logMessages ?? []))

    expect(findLogMessage('proof is valid', ...txs.filter((t) => t !== null))).toBeDefined()
    checkValidatationResult(newSigner, 11_155_420, 'op-event-v2.json')
  })

  it('errors out if cache account limit is reached', async () => {
    const newSigner = await generateAndFundNewSigner()

    // the proof cache account is currently capped to 3000 bytes. So these txs should sucdeed. The data
    // we are sending does not mater
    for (let i = 0; i < 5; i++) {
      const sig = await program.methods
        .loadProof(proof.subarray(0, 600))
        .accounts({ authority: newSigner.publicKey })
        .signers([newSigner])
        .rpc(confirmOptions)

      const tx = await provider.connection.getTransaction(sig, {
        maxSupportedTransactionVersion: 0,
        commitment: 'confirmed',
      })

      console.log(tx?.meta?.logMessages ?? [])
    }

    try {
      // now that the cache account is full, sending one more byte will trigger an error
      const sig = await program.methods
        .loadProof(proof.subarray(0, 1))
        .accounts({ authority: newSigner.publicKey })
        .signers([newSigner])
        .rpc(confirmOptions)

      const tx = await provider.connection.getTransaction(sig, {
        maxSupportedTransactionVersion: 0,
        commitment: 'confirmed',
      })
      console.log(tx)
      throw new Error('loadProof should have failed')
    } catch (err: any) {
      expect(err instanceof anchor.AnchorError)
      expect(err.logs).toBeDefined()
      expect(err.logs!.find((log: string) => log.includes('AnchorError caused by account: cache_account')))
    }
  })

  it('clears cache', async () => {
    const newSigner = await generateAndFundNewSigner()
    const cacheAccount = findProgramAddress([Buffer.from('cache'), newSigner.publicKey.toBuffer()], program.programId)

    // the cache account should be empty at this point
    const cache0 = await program.account.proofCacheAccount.fetch(cacheAccount, 'confirmed')
    expect(cache0.cache.length).toEqual(0)

    await program.methods
      .loadProof(proof.subarray(0, 600))
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions)

    // confirm that the proof chunk has been loaded
    const cache1 = await program.account.proofCacheAccount.fetch(cacheAccount, 'confirmed')
    expect(cache1.cache.length).toEqual(600)

    await program.methods
      .clearProofCache()
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions)

    // confirm the cache has been cleared
    const cache2 = await program.account.proofCacheAccount.fetch(cacheAccount, 'confirmed')
    expect(cache2.cache.length).toEqual(0)
  })

  it('runs proverctl', async () => {
    const newSigner = await generateAndFundNewSigner()

    const cacheAccount = findProgramAddress([Buffer.from('cache'), newSigner.publicKey.toBuffer()], program.programId)

    await program.methods
      .loadProof(proof.subarray(0, 600))
      .accounts({ authority: newSigner.publicKey })
      .signers([newSigner])
      .rpc(confirmOptions)

    // confirm that the proof chunk has been loaded
    const cache1 = await program.account.proofCacheAccount.fetch(cacheAccount, 'confirmed')
    expect(cache1.cache.length).toEqual(600)

    const clearCacheOutput = runProverCtl('--keypair', bs58.encode(newSigner.secretKey), 'clear-cache')
    expect(clearCacheOutput.includes('proof cache successfully cleared'))

    // confirm the cache has been cleared
    const cache2 = await program.account.proofCacheAccount.fetch(cacheAccount, 'confirmed')
    expect(cache2.cache.length).toEqual(0)
  })

  it('support cpi calls', async () => {
    const newSigner = await generateAndFundNewSigner()
    const cacheAccount = findProgramAddress([Buffer.from('cache'), newSigner.publicKey.toBuffer()], program.programId)
    const resultAccount = findProgramAddress([Buffer.from('result'), newSigner.publicKey.toBuffer()], program.programId)

    await cpiclient.methods
      .callLoadProof()
      .accounts({
        authority: newSigner.publicKey,
        cacheAccount: cacheAccount,
      })
      .signers([newSigner])
      .rpc(confirmOptions)

    const signature = await cpiclient.methods
      .callValidateEvent()
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 })])
      .accounts({
        authority: newSigner.publicKey,
        cacheAccount: cacheAccount,
        internal: findProgramAddress([Buffer.from('internal')], program.programId),
        resultAccount: resultAccount,
      })
      .signers([newSigner])
      .rpc(confirmOptions)

    const tx = await provider.connection.getTransaction(signature, {
      maxSupportedTransactionVersion: 0,
      commitment: 'confirmed',
    })

    console.log(tx?.meta?.logMessages ?? [])

    expect(findLogMessage('proof is valid', tx!)).toBeDefined()
    expect(
      findLogMessage(
        'proof validated: chain_id: 11155420, emitting_contract: 0xf221750e52aa080835d2957f2eed0d5d7ddd8c38',
        tx!,
      ),
    ).toBeDefined()
  })

  it('runs mars', async () => {
    const data = 'foo bar zoo'
    const signer = await generateAndFundNewSigner()

    await mars.methods.initialize().accounts({ user: signer.publicKey }).signers([signer]).rpc(confirmOptions)

    const dataAccount = findProgramAddress([signer.publicKey.toBuffer()], mars.programId)

    const sig = await mars.methods.setData({ data: data }).accounts({ data: dataAccount }).rpc(confirmOptions)

    const tx = await provider.connection.getTransaction(sig, {
      maxSupportedTransactionVersion: 0,
      commitment: 'confirmed',
    })

    console.log(tx?.meta?.logMessages ?? [])

    const msg = findLogMessage('Prove', tx!)
    expect(msg.includes(`Prove: program: ${mars.programId}, data: ${data}`)).toBe(true)

    const account = await mars.account.data.fetch(dataAccount)
    expect(data).toEqual(account.data.toString())
  })

  function findProgramAddress(seeds: Array<Buffer | Uint8Array>, programId: PublicKey): PublicKey {
    const [address, _bump] = PublicKey.findProgramAddressSync(seeds, programId)
    return address
  }

  async function checkValidatationResult(signer: Keypair, chainId: number, eventFileName: string) {
    const resultAccount = findProgramAddress([Buffer.from('result'), signer.publicKey.toBuffer()], program.programId)
    const result = await program.account.validationResultAccount.fetch(resultAccount, 'confirmed')
    expect(result.isValid)

    let topics = Buffer.alloc(0)
    const expectedEvent = readEventFile(eventFileName)
    expectedEvent.topics.map((topic: string) => (topics = Buffer.concat([topics, Buffer.from(topic.slice(2), 'hex')])))

    expect(chainId).toEqual(result.chainId)
    expect(expectedEvent.address.slice(2)).toEqual(Buffer.from(result.emittingContract).toString('hex'))
    expect(Buffer.from(topics).toString('hex')).toEqual(Buffer.from(result.topics).toString('hex'))
    expect(expectedEvent.data.slice(2)).toEqual(Buffer.from(result.unindexedData).toString('hex'))
  }

  function readProofFile(fileName: string): Buffer<ArrayBuffer> {
    const content = fs.readFileSync(path.join(testDataPath, fileName), 'utf-8')
    return Buffer.from(content.trim().slice(2), 'hex')
  }

  function readEventFile(fileName: string): any {
    return JSON.parse(fs.readFileSync(path.join(testDataPath, fileName), 'utf-8'))
  }

  async function generateAndFundNewSigner(): Promise<Keypair> {
    const signer = Keypair.generate()
    const latestBlockhash = await provider.connection.getLatestBlockhash()

    const signature = await provider.connection.requestAirdrop(signer.publicKey, 2 * LAMPORTS_PER_SOL)
    await provider.connection.confirmTransaction(
      {
        signature,
        blockhash: latestBlockhash.blockhash,
        lastValidBlockHeight: latestBlockhash.lastValidBlockHeight,
      },
      'confirmed',
    )

    // must create accounts first
    await program.methods
      .createAccounts()
      .accounts({ authority: signer.publicKey })
      .signers([signer])
      .rpc(confirmOptions)

    return signer
  }

  function findLogMessage(needle: string, ...txs: VersionedTransactionResponse[]): string {
    for (const tx of txs) {
      if (tx?.meta?.logMessages === undefined) continue
      for (const logMessage of tx.meta!.logMessages!) {
        if (logMessage.includes(needle)) return logMessage
      }
    }
    throw Error(`string '${needle}' not found`)
  }

  function runProverCtl(...args: string[]): string {
    try {
      args = ['--program-keypair', programKeypairFile, ...args]
      const output = execSync(`cargo run --quiet --bin proverctl -- ${args.join(' ')} 2>&1`)
      return output.toString()
    } catch (err: any) {
      console.error('Error message:', err.message)
      if (err.stdout) console.error('stdout:\n', err.stdout.toString())
      if (err.stderr) console.error('stderr:\n', err.stderr.toString())
      if (err.status !== undefined) console.error('Exit code:', err.status)
      throw err
    }
  }
})
