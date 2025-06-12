#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    program::set_return_data, sysvar, sysvar::instructions::load_current_index_checked,
    sysvar::instructions::load_instruction_at_checked,
};
use borsh::BorshDeserialize;

pub mod instructions;

use instructions::*;

const DISCRIMINATOR_SIZE: usize = 8;

// This program ID is used as is when deploying the program to solana devnet and used from our
// staging env.
// We changing to something else when deploying to solana devnet (again) so we can use it from our
// production env and when this is deployed to solana mainnet.
// See the ./scripts/build-release.sh script and .github/actions/build/action.yaml action
declare_id!("FtdxWoZXZKNYn1Dx9XXDE5hKXWf69tjFJUofNZuaWUH3");

#[derive(Accounts)]
pub struct Initialize<'info> {
    // user will be the owner of the internal account
    #[account(mut, signer)]
    pub authority: Signer<'info>,

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

    /// CHECK: we need to access the internal account to get the client type and signer address,
    /// which are unique to the program instance and required to validate the proof
    #[account(
        seeds = [b"internal"],
        bump,
    )]
    pub internal: Account<'info, InternalAccount>,

    /// CHECK: this is only used to verify whether the program is being called from off-chain or
    /// CPI
    #[account(address = sysvar::instructions::ID)]
    pub instructions: AccountInfo<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct ProofCacheAccount {
    #[max_len(3000)]
    pub cache: Vec<u8>,
}

#[derive(Accounts)]
pub struct LoadProof<'info> {
    /// user will be the owner of the pda account
    #[account(mut, signer)]
    pub authority: Signer<'info>,

    /// CHECK: here's where the proof chunks are stored
    #[account(
        init_if_needed,
        seeds = [b"cache", authority.key().as_ref()],
        bump,
        payer = authority,
        space = DISCRIMINATOR_SIZE + ProofCacheAccount::INIT_SPACE,
    )]
    pub cache_account: Account<'info, ProofCacheAccount>,

    // need this to create the pda account
    pub system_program: Program<'info, System>,
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

    #[event]
    pub struct ValidateEventEvent {
        pub chain_id: u32,
        pub emitting_contract: [u8; 20],
        pub topics: Vec<u8>,
        pub unindexed_data: Vec<u8>,
    }

    pub fn initialize(
        ctx: Context<Initialize>,
        client_type: String,
        signer_addr: [u8; 20],
        peptide_chain_id: u64,
    ) -> Result<()> {
        let internal = &mut ctx.accounts.internal;

        // TODO validate client type
        // TODO: check account permissions (see ai conversation)

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

        // Determine if the current instruction is a Cross-Program Invocation (CPI)
        // by comparing the instruction's program_id to the current program's ID.
        // If they differ, the instruction was invoked by another program.
        let is_cpi = {
            let ix = ctx.accounts.instructions.to_account_info();
            let index = load_current_index_checked(&ix)? as usize;
            let current_ix = load_instruction_at_checked(index, &ix)?;
            current_ix.program_id != *ctx.program_id
        };

        // if we are being called by another program, set the return data so they can pick it up
        // otherwise (we are being called by an off-chain agent) emit an event for them to parse
        if is_cpi {
            set_return_data(&result.try_to_vec()?);
        } else if let ValidateEventResult::Valid(chain_id, event) = result {
            emit!(ValidateEventEvent {
                chain_id,
                topics: event.topics,
                emitting_contract: *event.emitting_contract.as_bytes(),
                unindexed_data: event.unindexed_data,
            })
        }

        //        trace!("clearing cache!");
        ctx.accounts.cache_account.cache.clear();

        Ok(())
    }

    // pub fn parse_event(_ctx: Context<ParseEvent>, event: Vec<u8>, num_topics: usize) -> Result<()> {
    //     // TODO
    //     // set_return_data(key.as_bytes());
    //     // Ok(parse_event::handler(event.as_slice(), num_topics)?)
    //     Ok(())
    // }
    //
}
