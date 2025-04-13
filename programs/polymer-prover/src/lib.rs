use anchor_lang::prelude::*;
use borsh::BorshDeserialize;

pub mod instructions;

use instructions::*;

const DISCRIMINATOR_SIZE: usize = 8;

declare_id!("8zQzyWLSgLFpm2Si6HASkYidyL2paQaLZGADAQ5mSyPz");

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = DISCRIMINATOR_SIZE + InternalAccount::INIT_SPACE,
        seeds = [b"internal"],
        bump,
    )]
    pub internal: Account<'info, InternalAccount>,

    #[account(mut, signer)]
    pub authority: Signer<'info>,
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
    #[account(
        mut,
        seeds = [authority.key().as_ref()],
        bump,
    )]
    pub cache_account: Account<'info, ProofCacheAccount>,
    #[account(mut, signer)]
    // user will be the owner of the pda account
    pub authority: Signer<'info>,

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

#[derive(Accounts)]
pub struct LoadProof<'info> {
    #[account(
        init_if_needed,
        seeds = [authority.key().as_ref()],
        bump,
        payer = authority,
        space = DISCRIMINATOR_SIZE + ProofCacheAccount::INIT_SPACE,
    )]
    pub cache_account: Account<'info, ProofCacheAccount>,
    #[account(mut, signer)]
    // user will be the owner of the pda account
    pub authority: Signer<'info>,
    // need this to create the pda account
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResizeProofCache<'info> {
    #[account(
        mut ,
        realloc = DISCRIMINATOR_SIZE + ProofCacheAccount::INIT_SPACE,
        realloc::payer = authority,
        realloc::zero = false,
        seeds = [authority.key().as_ref()],
        bump,
    )]
    pub cache_account: Account<'info, ProofCacheAccount>,
    #[account(mut, signer)]
    pub authority: Signer<'info>,
    // need this to mutate the pda account
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClearProofCache<'info> {
    #[account(
        mut ,
        seeds = [authority.key().as_ref()],
        bump,
    )]
    pub cache_account: Account<'info, ProofCacheAccount>,
    #[account(mut, signer)]
    pub authority: Signer<'info>,
    // need this to mutate the pda account
    pub system_program: Program<'info, System>,
}
// #[derive(Accounts)]
// pub struct ParseEvent {}

#[program]
pub mod polymer_prover {

    use instructions::validate_event::ValidateEventResult;

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

        Ok(())
    }

    pub fn resize_proof_cache(_ctx: Context<ResizeProofCache>) -> Result<()> {
        msg!("proof cache successfully resized");
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
        if let ValidateEventResult::Valid(chain_id, event) = result {
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
