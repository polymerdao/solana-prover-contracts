#![allow(unexpected_cfgs)]
use anchor_lang::prelude::*;

declare_id!("5d9Z6bsfZg8THSus5mtfpr5cF9eNQKqaPZWkUHMjgk6u");

const DISCRIMINATOR_SIZE: usize = 8;

#[program]
mod mars {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        msg!("Prove: Initialized");
        Ok(())
    }

    pub fn set_data(ctx: Context<SetData>, data: Data) -> Result<()> {
        let data_account = &mut ctx.accounts.data;
        data_account.data = data.data;
        // DO NOT REMOVE
        // this msg is what we end up generating a proof for!
        msg!("Prove: Data: {}", data_account.data);
        Ok(())
    }
}

#[account]
#[derive(Default, InitSpace)]
pub struct Data {
    #[max_len(32)]
    pub data: String,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init_if_needed,
        seeds = [user.key().as_ref()],
        bump,
        payer = user,
        space = DISCRIMINATOR_SIZE + Data::INIT_SPACE
    )]
    pub data: Account<'info, Data>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetData<'info> {
    #[account(mut)]
    pub data: Account<'info, Data>,
}
