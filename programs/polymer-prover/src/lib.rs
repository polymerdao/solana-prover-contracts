use anchor_lang::prelude::*;

pub mod instructions;
pub mod state;

use instructions::*;
use state::*;

declare_id!("B9cX6RY34xmsvSYwNfrzvtbuQTfoUw7ah6qAgawgwYEL");

#[program]
pub mod polymer_prover {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        client_type: String,
        signer_addr: [u8; 20],
        peptide_chain_id: u64,
    ) -> Result<()> {
        let account = &mut ctx.accounts.event_account;

        // TODO validate client type
        // TODO: check account permissions (see ai conversation)

        account.client_type = client_type;
        account.signer_addr = signer_addr;
        account.peptide_chain_id = peptide_chain_id;

        Ok(())
    }

    pub fn validate_event(ctx: Context<ValidateEvent>, proof: Vec<u8>) -> Result<()> {
        let account = &ctx.accounts.event_account;
        Ok(validate_event::handler(
            &account.client_type,
            &account.signer_addr,
            account.peptide_chain_id,
            proof,
        )?)
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = user,
        space = EventAccount::MAX_SIZE,
    )]
    pub event_account: Account<'info, EventAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ValidateEvent<'info> {
    #[account()]
    pub event_account: Account<'info, EventAccount>,
}

#[error_code]
pub enum ProverError {
    #[msg("The provided signature is invalid.")]
    InvalidSignature,
    #[msg("The signer is unknown.")]
    UnknownSigner,
    #[msg("The provided address is invalid.")]
    InvalidAddress,
}
