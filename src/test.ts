import * as anchor from '@coral-xyz/anchor'
import { Program } from '@coral-xyz/anchor'
import type { PolymerProver } from '../target/types/polymer_prover'
import type { CpiClient } from '../target/types/cpi_client'
import type { Mars } from '../target/types/mars'
import {
  type ConfirmOptions,
  type VersionedTransactionResponse,
  Keypair,
  PublicKey,
  LAMPORTS_PER_SOL,
} from '@solana/web3.js'
import { describe, it, beforeAll, expect } from 'bun:test'

describe('localnet', () => {
  const provider = anchor.AnchorProvider.env()
  anchor.setProvider(provider)
  const program = anchor.workspace.polymer_prover as Program<PolymerProver>
  const cpiclient = anchor.workspace.cpi_client as Program<CpiClient>
  const mars = anchor.workspace.mars as Program<Mars>

  const wallet = provider.wallet as anchor.Wallet
  const confirmOptions: ConfirmOptions = { commitment: 'confirmed' }

  beforeAll(async () => {
    console.log(`WALLET:            ${wallet.publicKey.toBase58()}`)
    console.log(`POLYMER PROVER ID: ${program.programId}`)
    console.log(`CPI CLIENT ID:     ${cpiclient.programId}`)
    console.log(`MARS ID:           ${mars.programId}`)
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

    console.log(`tx.signature: ${tx?.transaction.signatures}`)
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
})
