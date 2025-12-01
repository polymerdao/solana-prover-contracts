#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use borsh::BorshDeserialize;

pub mod instructions;

use instructions::*;

const DISCRIMINATOR_SIZE: usize = 8;

// This program ID is used when deploying the program to solana mainnet and used from our
// testnet and mainnet envs.
// For devnet and shadownet, we use FtdxWoZXZKNYn1Dx9XXDE5hKXWf69tjFJUofNZuaWUH3
// The magic happens in ./scripts/build-release.sh script and .github/actions/build/action.yaml action
declare_id!("CdvSq48QUukYuMczgZAVNZrwcHNshBdtqrjW26sQiGPs");

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// require to signatures to prevent initialization frontrunning
    #[account(mut, signer)]
    pub authority: Signer<'info>,

    /// this prevents frontrunning of the initialization
    #[account(address = crate::ID)]
    pub program: Signer<'info>,

    /// hold the internal fields that need to be used during the proof validation
    #[account(
        init,
        payer = authority,
        space = DISCRIMINATOR_SIZE + InternalAccount::INIT_SPACE,
        seeds = [b"internal"],
        bump,
    )]
    pub internal: Account<'info, InternalAccount>,

    /// required to create the pda account
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct InternalAccount {
    /// store the owner's public key
    pub authority: Pubkey,

    /// Client type used on peptide to generate the proof. It is part of the proof key
    #[max_len(32)]
    pub client_type: String,

    /// Known signer address that signed the peptide state root
    pub signer_addr: [u8; 20],

    // Peptide chain ID included in the proof
    pub peptide_chain_id: u64,
}

#[derive(Accounts)]
pub struct ValidateEvent<'info> {
    // user will be the owner of the pda account
    #[account(mut, signer)]
    pub authority: Signer<'info>,

    /// CHECK: the proof lives in this account. It would have been loaded into via the
    /// LoadProof instruction
    #[account(
        mut,
        seeds = [b"cache", authority.key().as_ref()],
        bump,
    )]
    pub cache_account: Account<'info, ProofCacheAccount>,

    /// CHECK: the result of the validation will be stored in this account.
    #[account(
        mut,
        seeds = [b"result", authority.key().as_ref()],
        bump,
    )]
    pub result_account: Account<'info, ValidationResultAccount>,

    /// CHECK: we need to access the internal account to get the client type and signer address,
    /// which are unique to the program instance and required to validate the proof
    #[account(
        seeds = [b"internal"],
        bump,
    )]
    pub internal: Account<'info, InternalAccount>,
}

#[account]
#[derive(InitSpace)]
pub struct ProofCacheAccount {
    #[max_len(3000)]
    pub cache: Vec<u8>,
}

#[account]
#[derive(InitSpace, Default)]
pub struct ValidationResultAccount {
    /// whether the proof is valid or not
    pub is_valid: bool,

    /// error message if the proof is not valid
    #[max_len(64)]
    pub error_message: String,

    /// the chain ID of the event that was validated
    pub chain_id: u32,

    /// the emitting contract address that emitted the event
    pub emitting_contract: [u8; 20],

    #[max_len(32 * 4)] // 32 bytes per topic, max 4 topics
    pub topics: Vec<u8>,

    /// the unindexed data of the event that was validated
    #[max_len(3000)]
    pub unindexed_data: Vec<u8>,
}

#[derive(Accounts)]
pub struct CreateAccounts<'info> {
    // user will be the owner of the pda accounts
    #[account(mut, signer)]
    pub authority: Signer<'info>,

    /// CHECK: this is used to store the chunks of the proof to be validated later. We have to do
    /// it this way because the proof is too large to be passed as an argument to the instruction
    /// itself thanks to the transaction size limit.
    #[account(
        init,
        seeds = [b"cache", authority.key().as_ref()],
        bump,
        payer = authority,
        space = DISCRIMINATOR_SIZE + ProofCacheAccount::INIT_SPACE,
    )]
    pub cache_account: Account<'info, ProofCacheAccount>,

    /// CHECK: this is used to store the result of the validation. We have to do it this way
    /// because the result may be too large to be emitted as an event or through the return data
    #[account(
        init,
        seeds = [b"result", authority.key().as_ref()],
        bump,
        payer = authority,
        space = DISCRIMINATOR_SIZE + ValidationResultAccount::INIT_SPACE,
    )]
    pub result_account: Account<'info, ValidationResultAccount>,

    // need this to create the pda account
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CloseAccounts<'info> {
    // user will be the owner of the pda accounts
    #[account(mut, signer)]
    pub authority: Signer<'info>,

    /// CHECK: close the cache account and transfer its lamports to the authority
    #[account(
        mut,
        close = authority,
        seeds = [b"cache", authority.key().as_ref()],
        bump,
    )]
    pub cache_account: Account<'info, ProofCacheAccount>,

    /// CHECK: close the result account and transfer its lamports to the authority
    #[account(
        mut,
        close = authority,
        seeds = [b"result", authority.key().as_ref()],
        bump,
    )]
    pub result_account: Account<'info, ValidationResultAccount>,
}

#[derive(Accounts)]
pub struct LoadProof<'info> {
    /// user will be the owner of the pda account
    #[account(mut, signer)]
    pub authority: Signer<'info>,

    /// CHECK: here's where the proof chunks are stored
    #[account(
        mut,
        seeds = [b"cache", authority.key().as_ref()],
        bump,
    )]
    pub cache_account: Account<'info, ProofCacheAccount>,
}

#[derive(Accounts)]
pub struct ClearProofCache<'info> {
    /// user will be the owner of the pda account
    #[account(mut, signer)]
    pub authority: Signer<'info>,

    /// CHECK: this is used to store the chunks of the proof to be validated later. We have to do
    #[account(
        mut ,
        seeds = [b"cache", authority.key().as_ref()],
        bump,
    )]
    pub cache_account: Account<'info, ProofCacheAccount>,

    // need this to mutate the pda account
    pub system_program: Program<'info, System>,
}

#[program]
pub mod polymer_prover {

    use instructions::validate_event::ValidateEventResult;

    use crate::instructions::parse_event::EthAddress;

    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        client_type: String,
        signer_addr: [u8; 20],
        peptide_chain_id: u64,
    ) -> Result<()> {
        let internal = &mut ctx.accounts.internal;

        internal.authority = ctx.accounts.authority.key();
        internal.client_type = client_type;
        internal.signer_addr = signer_addr;
        internal.peptide_chain_id = peptide_chain_id;

        msg!("client_type: {}", internal.client_type);
        msg!("peptide_chain_id: {}", internal.peptide_chain_id);
        msg!(
            "signer_addr: {}",
            EthAddress::from_bytes(&internal.signer_addr).to_hex()
        );
        Ok(())
    }

    pub fn create_accounts(_ctx: Context<CreateAccounts>) -> Result<()> {
        msg!("accounts successfully created");
        Ok(())
    }

    pub fn close_accounts(_ctx: Context<CloseAccounts>) -> Result<()> {
        msg!("accounts successfully closed");
        Ok(())
    }

    pub fn clear_proof_cache(ctx: Context<ClearProofCache>) -> Result<()> {
        msg!("proof cache successfully cleared");
        ctx.accounts.cache_account.cache.clear();
        Ok(())
    }

    pub fn load_proof(ctx: Context<LoadProof>, proof_chunk: Vec<u8>) -> Result<()> {
        ctx.accounts.cache_account.cache.extend(proof_chunk.iter());
        Ok(())
    }

    pub fn validate_event(ctx: Context<ValidateEvent>) -> Result<()> {
        // this is set by the owner/deployer during initialize()
        let internal = &ctx.accounts.internal;

        let result = validate_event::handler(
            &ctx.accounts.cache_account.cache,
            &internal.client_type,
            &internal.signer_addr,
            internal.peptide_chain_id,
        );

        msg!("{}", result);

        // create a new ValidationResultAccount to store the result of the validation
        let mut out = ValidationResultAccount::default();

        // if the result is valid, we store the event data in the result account
        // if the result is invalid, we store the error message in the result account
        if let ValidateEventResult::Valid(chain_id, event) = result {
            out.is_valid = true;
            out.chain_id = chain_id;
            out.emitting_contract = *event.emitting_contract.as_bytes();
            out.topics = event.topics.clone();
            out.unindexed_data = event.unindexed_data.clone();
        } else {
            out.is_valid = false;
            out.error_message = result.to_string();
        }

        ctx.accounts.result_account.set_inner(out);
        ctx.accounts.cache_account.cache.clear();

        Ok(())
    }
}
