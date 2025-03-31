use anchor_lang::prelude::*;

pub mod error;
pub mod instructions;
pub mod state;

use instructions::*;
use state::*;

declare_id!("B9cX6RY34xmsvSYwNfrzvtbuQTfoUw7ah6qAgawgwYEL");

#[program]
pub mod polymer_prover {

    use super::*;

    #[derive(AnchorSerialize, AnchorDeserialize)]
    pub struct ParsedEvent {
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
        let account = &mut ctx.accounts.event_account;

        // TODO validate client type
        // TODO: check account permissions (see ai conversation)

        account.client_type = client_type;
        account.signer_addr = signer_addr;
        account.peptide_chain_id = peptide_chain_id;

        Ok(())
    }

    pub fn validate_event(ctx: Context<ValidateEvent>, proof: Vec<u8>) -> Result<ParsedEvent> {
        let account = &ctx.accounts.event_account;

        match validate_event::handler(
            &account.client_type,
            &account.signer_addr,
            account.peptide_chain_id,
            proof,
        ) {
            Ok(event) => Ok(ParsedEvent {
                emitting_contract: *event.emitting_contract.as_bytes(),
                unindexed_data: event.unindexed_data,
                topics: event.topics,
            }),
            Err(e) => Err(e.into()),
        }
    }

    // pub fn parse_event(_ctx: Context<ParseEvent>, event: Vec<u8>, num_topics: usize) -> Result<()> {
    //     // TODO
    //     // set_return_data(key.as_bytes());
    //     // Ok(parse_event::handler(event.as_slice(), num_topics)?)
    //     Ok(())
    // }
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

#[derive(Accounts)]
pub struct ParseEvent {}
